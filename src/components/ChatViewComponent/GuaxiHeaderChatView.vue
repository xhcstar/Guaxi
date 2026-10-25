<template>
  <div class="guaxi-header">
    <!-- 左侧：Logo 和 Icon -->
    <div class="header-left">
      <GuaxiSiderToggle view-key="chat" side="left" />
      <div class="logo-text">Guaxi</div>
      <ChatModelSelector class="model-selector" />
    </div>

    <!-- 中间：搜索框 (占位实现) -->
    <div class="header-center">
      <div class="search-container">
        <n-input round clearable placeholder="搜索当前会话消息..." v-model:value="chatMessageSearchQuery" @focus="showSearchPlaceholder">
          <template #prefix>
            <n-icon :size="16">
              <Icon :icon="searchIcon" width="16" height="16" />
            </n-icon>
          </template>
        </n-input>
      </div>
    </div>

    <!-- 右侧：功能图标 -->
    <div class="header-right">
      <n-icon class="header-icon" size="20" @click="openWebModel">
        <Icon :icon="externalLinkIcon" width="18" height="18" />
      </n-icon>
      <GuaxiSiderToggle view-key="chat" side="right" />
    </div>
  </div>
</template>

<script setup>
import { createLogger } from '../../utils/logger'
import { useMessage } from 'naive-ui'
import { invoke } from '@tauri-apps/api/core'
import { useChatStore } from '../../stores/chatStore'
import { computed } from 'vue'
import ChatModelSelector from "./ChatModelSelector.vue"
import GuaxiSiderToggle from '../GuaxiSiderToggle.vue'
import { Icon } from '@iconify/vue'
import searchIcon from '@iconify-icons/lucide/search'
import externalLinkIcon from '@iconify-icons/lucide/external-link'

const logger = createLogger('GuaxiHeaderChatView')
const chatStore = useChatStore();
const message = useMessage();
const chatMessageSearchQuery = computed({
  get: () => chatStore.chatMessageSearchQuery,
  set: (v) => chatStore.setChatMessageSearchQuery(v),
})


const showSearchPlaceholder = () => {
  logger.info('搜索框获得焦点 - 占位功能');
};

const showNotes = () => {
  logger.info('显示笔记点击');
};

const CaptureScreenshot = () => {
  logger.info('截图功能点击');
};


const openWebModel = async () => {
  try {
    const modelId = chatStore.selectedModel
    if (modelId !== 7 && modelId !== 8) {
      message.warning('当前模型不是网页模式')
      return
    }
    const provider = modelId === 7 ? 'yuanbao' : 'chatgpt'
    await invoke('open_web_model_window', { provider })
  } catch (e) {
    logger.error('打开网页模型失败:', e)
    message.error('打开网页模型失败')
  }
}
</script>

<style scoped>
.guaxi-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 35px;
  background-color: var(--guaxi-header-bg);
  box-shadow: var(--guaxi-header-shadow);
  border-bottom: 2px solid var(--guaxi-border);
  padding: 0 8px;
  box-sizing: border-box;
}

/* 左侧样式 */
.header-left {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 200px;
}

.logo-text {
  font-weight: bold;
  font-size: 16px;
  white-space: nowrap;
}

.model-selector {
  margin-left: 16px;
}

/* 中间样式 */
.header-center {
  flex: 1;
  display: flex;
  justify-content: center;
  max-width: 400px;
  margin: 0 10px;
}

.search-container {
  width: 100%;
}

/* 右侧样式 */
.header-right {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 200px;
  justify-content: flex-end;
}

/* 图标通用样式 */
.header-icon {
  flex: 0 0 auto;
  cursor: pointer;
  padding: 3px;
  border-radius: 6px;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--guaxi-text-secondary);
  transition: color 0.16s ease, background-color 0.16s ease;
}

.header-icon:hover {
  color: var(--guaxi-text-primary);
  background-color: rgba(127, 127, 127, 0.16);
}

/* 输入框样式 */
.search-container :deep(.n-input) {
  height: 25px !important;
  min-height: 25px !important;
  margin: 0 !important;
}

.search-container :deep(.n-input .n-input-wrapper) {
  background-color: var(--guaxi-panel-bg) !important;
}

.search-container :deep(.n-input .n-input__border),
.search-container :deep(.n-input .n-input__state-border) {
  border-color: var(--guaxi-border) !important;
}

.search-container :deep(.n-input__input-el) {
  color: var(--guaxi-text-primary) !important;
}

.search-container :deep(.n-input__input-el::placeholder) {
  color: var(--guaxi-text-secondary) !important;
}

.search-container :deep(.n-input__prefix),
.search-container :deep(.n-input__suffix) {
  color: var(--guaxi-text-secondary) !important;
}
</style>
