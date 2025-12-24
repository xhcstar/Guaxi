<template>
  <div class="guaxi-header">
    <!-- 左侧：Logo 和 Icon -->
    <div class="header-left">
      <n-icon size="20" class="header-icon" :component="PanelLeftContract24Regular" @click="toggleLeftSider"></n-icon>
      <div class="logo-text">Guaxi</div>
      <ChatModelSelector class="model-selector" />
    </div>

    <!-- 中间：搜索框 (占位实现) -->
    <div class="header-center">
      <div class="search-container">
        <n-input round clearable placeholder="搜索..." @focus="showSearchPlaceholder">
          <template #prefix>
            <n-icon :component="Search20Filled" />
          </template>
        </n-input>
      </div>
    </div>

    <!-- 右侧：功能图标 -->
    <div class="header-right">
      <n-icon class="header-icon" size="20" :component="TextBulletListLtr20Filled" @click="showNotes"></n-icon>
      <n-icon class="header-icon" size="20" :component="Share20Regular" @click="exportChatSession"></n-icon>
      <n-icon class="header-icon" size="20" :component="Open20Regular" @click="openWebModel"></n-icon>
      <n-icon class="header-icon" size="20" :component="Screenshot24Regular" @click="CaptureScreenshot"></n-icon>
      <n-icon class="header-icon" size="20" :component="PanelRightContract24Regular" @click="toggleRightSider"></n-icon>
    </div>
  </div>
</template>

<script setup>
import { computed } from "vue";
import { useMessage } from 'naive-ui'
import { invoke } from '@tauri-apps/api/core'
import { useSiderStore } from '../../stores/useSiderStore'
import { useChatStore } from '../../stores/chatStore'
import ChatModelSelector from "./ChatModelSelector.vue"
import {
  PanelLeftContract24Regular,
  PanelRightContract24Regular,
  Search20Filled,
  TextBulletListLtr20Filled,
  Screenshot24Regular,
  Share20Regular,
  Open20Regular,
} from '@vicons/fluent'

const viewKey = 'chat'
const siderStore = useSiderStore();
const chatStore = useChatStore();
const message = useMessage();

const activeChat = computed(() =>
  chatStore.SelectModelchatSessions.find((chatmodel) => chatmodel.id === chatStore.selectedModel)
);

const activeChatMessages = computed(() =>
  activeChat.value?.MessagesList.find((topic) => topic.topicid === chatStore.selectedTopic) || []
);

const toggleLeftSider = () => {
  siderStore.toggleLeftSider(viewKey)
};

const toggleRightSider = () => {
  siderStore.toggleRightSider(viewKey)
};

const showSearchPlaceholder = () => {
  console.log('搜索框获得焦点 - 占位功能');
};

const showNotes = () => {
  console.log('显示笔记点击');
};

const CaptureScreenshot = () => {
  console.log('截图功能点击');
};

const exportChatSession = async () => {
  try {
    const content = activeChatMessages.value?.topic || []
    const filePath = await invoke('export_chat_session', {
      request: {
        model_id: chatStore.selectedModel,
        model_name: activeChat.value?.name || '',
        topic_id: chatStore.selectedTopic,
        session_id: chatStore.sendChatSessionId || '',
        content,
      }
    })
    message.success(`已导出到: ${filePath}`)
  } catch (e) {
    console.error('导出会话失败:', e)
    message.error('导出会话失败')
  }
}

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
    console.error('打开网页模型失败:', e)
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
  background-color: #f5f5f5;
  box-shadow: 0px 2px 4px rgba(0, 0, 0, 0.1);
  border-bottom: 2px solid #aab4ae;
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
  cursor: pointer;
  transition: all 0.2s ease;
  padding: 2px;
  border-radius: 4px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.header-icon:hover {
  opacity: 0.8;
  transform: scale(1.05);
  background-color: rgba(0, 0, 0, 0.05);
}

/* 输入框样式 */
:deep(.n-input) {
  height: 25px !important;
  min-height: 25px !important;
  line-height: 25px !important;
}
</style>