<template>
  <div class="chat-container">
    <div class="chat-header">
      <n-avatar :src="activeChat?.ModelAvatar" size="medium" round />
      <span class="chat-title">{{ activeChat?.name || "未选择会话" }}</span>
    </div>

    <div class="message-list" ref="messageContainer">
      <div v-for="(message, index) in activeChatMessages.topic" :key="message.msgid" class="message-item"
        :class="message.sender === 'me' ? 'message-right' : 'message-left'">
        <n-avatar v-if="message.sender === 'me'" :src="activeChat.MeAvatar" size="medium" round />
        <n-avatar v-else :src="activeChat.ModelAvatar" size="medium" round />
        <div class="message-bubble">
          <div class="message-content">{{ message.content }}</div>
        </div>
      </div>

    </div>

    <div class="input-area">
      <div class="chat-input-height-handle" @mousedown="startResize"></div>
        <n-input type="textarea" placeholder="请输入您的问题..." v-model:value="messageInput" @keypress.enter="sendMessage"
        style="flex-grow: 1; height: 100%; padding: 2px; border-radius: 5px; box-sizing: border-box;"/>
        <n-button @click="sendMessage" type="primary"
          style="position: absolute; right: 20px; bottom: 20px; border-radius: 50%; padding: 0 20px; height: 36px; min-width: 36px; font-size: 12px;">
          发送
        </n-button>
    </div>
  </div>
</template>

<script setup>
import { ref, nextTick, computed, onMounted} from 'vue'
import { useChatStore } from "../../stores/chatStore";


const chatStore = useChatStore();
const messageInput = ref(""); // 用户输入的消息

// 当前激活的聊天会话
const activeChat = computed(() =>
  chatStore.SelectModelchatSessions.find((chatmodel) => chatmodel.id === chatStore.selectedModel)
);

// 当前会话的消息记录
const activeChatMessages = computed(() => activeChat.value?.MessagesList.find((topic) => topic.topicid === chatStore.selectedTopic) || []);

// 滚动到最新消息
const messageContainer = ref(null);
const scrollToBottom = () => {
  nextTick(() => {
    if (messageContainer.value) {
      messageContainer.value.scrollTop = messageContainer.value.scrollHeight;
    }
  });
};

// 发送消息
const sendMessage = () => {
  if (!messageInput.value.trim() || !activeChat.value) return;

  // 更新当前会话的消息记录
  // chatStore.updateChatSession(activeChat.value.id, messageInput.value);
  activeChatMessages.value.topic.push({
    msgid: activeChatMessages.value.topic.length + 1,
    sender: "me",
    content: messageInput.value.trim()
  });

  messageInput.value = ""; // 清空输入框
  scrollToBottom(); // 滚动到底部
};

// 初次加载后滚动到底部
onMounted(() => {
  scrollToBottom();
});

let startY = 0;
let startHeight = 0;

const startResize = (e) => {
  e.preventDefault(); // 防止选中文本

  const inputArea = document.querySelector('.input-area');
  startY = e.clientY; // 记录鼠标初始位置
  startHeight = inputArea.offsetHeight; // 记录初始高度

  // 添加监听器，实时跟踪鼠标位置
  document.addEventListener('mousemove', onMouseMove);
  document.addEventListener('mouseup', stopResize);

  // 禁用文本框的选中
  document.body.style.userSelect = 'none';
};

const onMouseMove = (e) => {
  const diff = startY - e.clientY;
  const newHeight = startHeight + diff;
  if (newHeight > 85) { // 限制最小高度
    const inputArea = document.querySelector('.input-area');
    inputArea.style.height = newHeight + 'px'; // 修改 height
  }
};

const stopResize = () => {
  // 移除鼠标移动和鼠标松开事件监听器
  document.removeEventListener('mousemove', onMouseMove);
  document.removeEventListener('mouseup', stopResize);

  // 恢复文本框的选中功能
  document.body.style.userSelect = '';
};

</script>

<style scoped>
.chat-container {
  width: 100%;
  height: 100%;
  border: 1px solid #ddd;
  display: flex;
  flex-direction: column;
  padding: 0;
  box-sizing: border-box;
}

.chat-header {
  display: flex;
  align-items: center;
  padding: 7px 15px;
  border-bottom: 1px solid #ddd;
  background-color: #f8f9fa;
}

.chat-title {
  margin-left: 10px;
  font-size: 14px;
  font-weight: bold;
  color: #333;
}

.message-list {
  flex-grow: 1;
  overflow-y: auto;
  padding: 15px;
  background-color: #ffffff;
}

.message-item {
  display: flex;
  align-items: center;
  margin-bottom: 12px;
}

.message-left {
  flex-direction: row;
}

.message-right {
  flex-direction: row-reverse;
}

.message-bubble {
  max-width: 70%;
  padding: 12px;
  border-radius: 20px;
  margin: 0 8px;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
}

.message-left .message-bubble {
  background-color: #f0f0f0;
  border-top-left-radius: 0;
}

.message-right .message-bubble {
  background-color: #007aff;
  color: white;
  border-top-right-radius: 0;
}

.input-area {
  margin-top: 1px;
  padding-top: 1px;
  border-top: 1px solid #ddd;
  position: relative;
  padding-bottom: 2px;
  height: 260px;
}

.chat-input-height-handle {
  position: relative;
  width: 100%;
  height: 1px;
  cursor: ns-resize;
  background-color: #aab4ae;
}
</style>
