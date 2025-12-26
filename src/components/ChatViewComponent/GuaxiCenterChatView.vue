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
          <div class="message-content" v-html="message.content" :ref="el => setMessageRef(el, index)"></div>
        </div>
      </div>

    </div>

    <div class="input-area">
      <div class="chat-input-height-handle" @mousedown="startResize"></div>
      <n-input
        type="textarea"
        placeholder="请输入您的问题..."
        v-model:value="messageInput"
        @keydown="handleKeydown"
        style="flex-grow: 1; height: 100%; padding: 2px; border-radius: 5px; box-sizing: border-box;   font-size: 17px;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace;"
      />
      <n-button @click="sendMessage" color="#dbe7e4"
        style="position: absolute; right: 10px; bottom: 8px; border-radius: 50%; padding: 0 20px; height: 36px; min-width: 36px; font-size: 12px;">
        <template #icon>
          <n-icon>
            <svg t="1737646310254" class="icon" viewBox="0 0 1178 1024" version="1.1" xmlns="http://www.w3.org/2000/svg"
              p-id="5954" width="48" height="48">
              <path
                d="M1145.732473 24.529036A96.502796 96.502796 0 0 0 1047.167948 6.302655L62.5449 378.38417a96.762677 96.762677 0 0 0-8.229592 177.44733l153.746105 75.175158a32.918368 32.918368 0 0 1 8.229592 5.804029c15.974071 41.130635 79.05606 202.292036 107.123301 260.540222a120.758435 120.758435 0 0 0 64.520002 59.530271c-1.611268-0.155929-3.465091-0.485113-5.318915-0.814297a88.359831 88.359831 0 0 0 14.518733 4.192761 96.190937 96.190937 0 0 0 87.59751-26.455973l57.260636-57.260636a32.485232 32.485232 0 0 1 38.722396-5.318915l246.662532 140.024344a97.143838 97.143838 0 0 0 47.748959 12.751536 97.022559 97.022559 0 0 0 94.042581-73.56389l205.999684-830.478457a95.671174 95.671174 0 0 0-29.453277-95.290014z m-868.005397 592.3747L805.56445 314.903695l-297.997861 305.222576a48.112794 48.112794 0 0 0-10.811085 17.325457c-0.155929 0.329184-0.155929 0.485113-0.329184 0.814297-0.641042 1.940451-59.53027 175.680134-94.371764 254.233756a58.404116 58.404116 0 0 1-20.651945-22.748325c-25.814931-54.211355-85.986243-207.628277-103.727511-252.795743z"
                fill="#6EC2B0" p-id="5955"></path>
            </svg>
          </n-icon>
        </template>
      </n-button>
    </div>
  </div>
</template>

<script setup>
import { ref, nextTick, computed, onMounted, onUnmounted, watch } from 'vue'
import { useChatStore } from "../../stores/chatStore";
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { addCopyButtons } from "../GuaxiMarkdownRender/GuaxiMarker";


const chatStore = useChatStore();
const messageInput = ref("");
const inputHistory = ref([]);
const historyIndex = ref(-1);
let unsubscribeStream = null;
let unsubscribeWebModel = null;
const messageRefs = ref([]);


// 当前激活的聊天会话
const activeChat = computed(() =>
  chatStore.SelectModelchatSessions.find((chatmodel) => chatmodel.id === chatStore.selectedModel)
);

// 当前会话的消息记录
const activeChatMessages = computed(() =>
  activeChat.value?.MessagesList.find((topic) => topic.topicid === chatStore.selectedTopic) || { topicid: chatStore.selectedTopic, topic: [] }
);

const autosaveTimer = ref(null);

const isWebModel = computed(() => chatStore.selectedModel === 7 || chatStore.selectedModel === 8);

watch(
  () => chatStore.selectedModel,
  async (modelId) => {
    if (modelId !== 7 && modelId !== 8) return;
    const provider = modelId === 7 ? 'yuanbao' : 'chatgpt';
    try {
      await invoke('prepare_web_model_window', { provider });
    } catch (e) {
      console.error('预加载网页模型窗口失败:', e);
    }
  }
);

const getTopicSessionId = (topic) => {
  const items = Array.isArray(topic) ? topic : [];
  for (let i = items.length - 1; i >= 0; i--) {
    const sid = items[i]?.chatSessionId;
    if (typeof sid === 'string' && sid.trim()) return sid;
  }
  return '';
};

watch(
  [() => chatStore.selectedModel, () => chatStore.selectedTopic],
  () => {
    const topic = Array.isArray(activeChatMessages.value?.topic) ? activeChatMessages.value.topic : [];
    chatStore.sendChatSessionId = getTopicSessionId(topic) || '';
  },
  { immediate: true }
);

const ensureTopicSessionId = (topic) => {
  const existing = getTopicSessionId(topic);
  const sid = existing || `${Date.now()}-${Math.random().toString(16).slice(2)}`;
  chatStore.sendChatSessionId = sid;
  return sid;
};

const saveSessionSnapshot = async () => {
  try {
    const content = Array.isArray(activeChatMessages.value?.topic) ? activeChatMessages.value.topic : [];
    const sid = getTopicSessionId(content) || chatStore.sendChatSessionId || null;
    await invoke('save_chat_session', {
      request: {
        model_id: chatStore.selectedModel,
        model_name: activeChat.value?.name || '',
        topic_id: chatStore.selectedTopic,
        session_id: sid,
        content,
      },
    });
  } catch (e) {
    console.error('自动保存会话失败:', e);
  }
};

const scheduleAutosave = () => {
  if (autosaveTimer.value) {
    clearTimeout(autosaveTimer.value);
  }
  autosaveTimer.value = setTimeout(() => {
    autosaveTimer.value = null;
    saveSessionSnapshot();
  }, 800);
};

// 滚动到最新消息
const messageContainer = ref(null);
const scrollToBottom = () => {
  nextTick(() => {
    if (messageContainer.value) {
      messageContainer.value.scrollTop = messageContainer.value.scrollHeight;
    }
  });
};

const askDeepSeek = async (question, sessionId) => {
  try {
    const request = {
      "modelid": chatStore.selectedModel,
      "question": question,
      "topicid": chatStore.selectedTopic,
      "sessionid": sessionId,
    };
    console.log('Request:', request);
    // const response = await invoke('get_chat_answer', { request });
    const response = await invoke('get_chat_answer_stream', { request });
    console.log('Response:', response);
    return response;
  } catch (error) {
    console.error('Error calling API:', error);
  }
};

// 发送消息
const sendMessage = async () => {
  if (!messageInput.value.trim() || !activeChat.value) return;

  const isWeb = isWebModel.value;

  const question = messageInput.value.trim();

  inputHistory.value.push(question);
  historyIndex.value = inputHistory.value.length;

  const topic = Array.isArray(activeChatMessages.value?.topic) ? activeChatMessages.value.topic : [];

  const sessionId = ensureTopicSessionId(topic);

  let provider = '';
  if (isWeb) {
    provider = chatStore.selectedModel === 7 ? 'yuanbao' : 'chatgpt';
  }

  if (
    topic.length === 1 &&
    topic[0]?.sender !== 'me' &&
    typeof topic[0]?.content === 'string' &&
    topic[0].content.includes('网页模式')
  ) {
    topic.splice(0, 1);
  }

  const myMsg = {
    msgid: topic.length + 1,
    sender: "me",
    chatSessionId: sessionId,
    raw: question,
    content: question,
  };

  topic.push(myMsg);
  const renderedP = chatStore.renderDebounced(question);
  if (renderedP && typeof renderedP.then === 'function') {
    renderedP.then((rendered) => {
      myMsg.content = rendered;
    });
  }

  messageInput.value = "";
  scrollToBottom();

  if (isWeb) {
    try {
      await invoke('web_model_send_message', {
        request: {
          provider,
          model_id: chatStore.selectedModel,
          topic_id: chatStore.selectedTopic,
          session_id: sessionId,
          content: question,
        },
      });
    } catch (e) {
      console.error('发送到网页模型失败:', e);
    }

    scheduleAutosave();
    return;
  }

  chatStore.setStreamTarget(chatStore.selectedModel, chatStore.selectedTopic);
  chatStore.accumulatedMessage = '';

  try {
    await askDeepSeek(question, sessionId);
  } catch (error) {
    console.error('Error during askDeepSeek:', error);
  }

  scrollToBottom();
  scheduleAutosave();
};

const handleKeydown = (event) => {
  const el = event?.target;

  if ((event.key === 'ArrowUp' || event.key === 'ArrowDown') && !event.shiftKey) {
    const hasSelectionApi = el && typeof el.selectionStart === 'number' && typeof el.selectionEnd === 'number';
    if (hasSelectionApi) {
      const atStart = el.selectionStart === 0 && el.selectionEnd === 0;
      const atEnd = el.selectionStart === el.value.length && el.selectionEnd === el.value.length;

      if (event.key === 'ArrowUp' && atStart && inputHistory.value.length > 0) {
        event.preventDefault();
        if (historyIndex.value === -1) {
          historyIndex.value = inputHistory.value.length;
        }
        historyIndex.value = Math.max(0, historyIndex.value - 1);
        messageInput.value = inputHistory.value[historyIndex.value] ?? '';
        requestAnimationFrame(() => {
          el.selectionStart = el.selectionEnd = el.value.length;
        });
        return;
      }

      if (event.key === 'ArrowDown' && atEnd && inputHistory.value.length > 0 && historyIndex.value !== -1) {
        event.preventDefault();
        historyIndex.value = Math.min(inputHistory.value.length, historyIndex.value + 1);
        messageInput.value = historyIndex.value >= inputHistory.value.length ? '' : (inputHistory.value[historyIndex.value] ?? '');
        requestAnimationFrame(() => {
          el.selectionStart = el.selectionEnd = el.value.length;
        });
        return;
      }
    }
  }

  if (event.key === 'Enter') {
    if (event.shiftKey) return;
    event.preventDefault();
    sendMessage();
  }
};

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
  let diff = startY - e.clientY;
  if (diff < 0) {
    diff = 0;
  }
  const newHeight = startHeight + diff;
  console.log('newHeight:', newHeight);
  if (newHeight > 85) { // 限制最小高度
    const inputArea = document.querySelector('.input-area');
    inputArea.style.height = newHeight + 'px'; // 修改 height
    // inputArea.style.height = `${Math.max(180, newHeight)}px`
  }
};

const stopResize = () => {
  // 移除鼠标移动和鼠标松开事件监听器
  document.removeEventListener('mousemove', onMouseMove);
  document.removeEventListener('mouseup', stopResize);

  // 恢复文本框的选中功能
  document.body.style.userSelect = '';
};

onMounted(() => {
  listen("modle-streaming-respond-output", (event) => {
    chatStore.collectChatSessionStream(event.payload);

    const topic = Array.isArray(activeChatMessages.value?.topic) ? activeChatMessages.value.topic : [];
    const latest = topic[topic.length - 1];
    if (latest && latest.sender !== 'me' && !latest.chatSessionId) {
      latest.chatSessionId = chatStore.sendChatSessionId || '';
    }

    scheduleAutosave();
  })
    .then((unsubFn) => {
      unsubscribeStream = unsubFn;
    })
    .catch((err) => {
      console.error('监听事件失败:', err);
      unsubscribeStream = null;
    });

  listen("web-model-chat-push", (event) => {
    const payload = event.payload || {};
    const modelId = payload.model_id;
    const topicId = payload.topic_id;
    const sessionId = payload.session_id;
    const title = payload.title;
    const messages = payload.messages;

    chatStore.appendMessagesToTopic({
      modelId,
      topicId,
      sessionId,
      title,
      messages,
    });

    if (modelId === chatStore.selectedModel && topicId === chatStore.selectedTopic) {
      scrollToBottom();
      scheduleAutosave();
    }
  })
    .then((unsubFn) => {
      unsubscribeWebModel = unsubFn;
    })
    .catch((err) => {
      console.error('监听 web-model-chat-push 失败:', err);
      unsubscribeWebModel = null;
    });
});

onUnmounted(() => {
  if (unsubscribeStream && typeof unsubscribeStream === 'function') {
    unsubscribeStream();
  }
  if (unsubscribeWebModel && typeof unsubscribeWebModel === 'function') {
    unsubscribeWebModel();
  }
  if (autosaveTimer.value) {
    clearTimeout(autosaveTimer.value);
    autosaveTimer.value = null;
  }
});

const setMessageRef = (el, index) => {
  if (el) {
    messageRefs.value[index] = el;
    nextTick(() => {
      addCopyButtons(el);
    });
  }
};

// watch(activeChatMessages, () => {
//   nextTick(() => {
//     messageRefs.value.forEach(container => {
//       if (container) {
//         addCopyButtons(container);
//       }
//     });
//   });
// }, { deep: true });

</script>

<style scoped>
.chat-container {
  width: 100%;
  height: 100%;
  border: 1px solid #ffffff;
  display: flex;
  flex-direction: column;
  padding: 0;
  box-sizing: border-box;
  min-height: 0;
}

.chat-header {
  display: flex;
  align-items: center;
  padding: 7px 15px;
  border-bottom: 1px solid #ffffff;
  background-color: #ffffff;
}

.chat-title {
  margin-left: 10px;
  font-size: 14px;
  font-weight: bold;
  color: #333;
}

.message-list {
  flex: 1 1 auto;
  overflow-y: auto;
  padding: 15px;
  background-color: #ffffff;
  min-height: 0;
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
  padding: 8px;
  border-radius: 20px;
  margin: 0 5px;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
}

.message-left .message-bubble {
  background-color: #fdfffe;
  font-size: 16px;
  font-family: 'Maple Mono NF CN', 'Noto Sans CJK SC', Consolas, 'Courier New', Courier, monospace;
  /* 使用等宽字体 */
  border-top-left-radius: 0;
}

.message-right .message-bubble {
  background-color: #45d6a6;
  font-size: 16px;
  font-family: 'Maple Mono NF CN', 'Noto Sans CJK SC', Consolas, 'Courier New', Courier, monospace;
  /* 使用等宽字体 */
  color: white;
  border-top-right-radius: 0;
}

.input-area {
  flex: 0 0 auto;
  border-top: 1px solid #ddd;
  position: relative;
  background-color: #ffffff;
  padding-bottom: 2px;
  height: 80px;
  min-height: 80px;
  font-size: 17px;
  font-family: 'Maple Mono NF CN', 'Noto Sans CJK SC', Consolas, 'Courier New', Courier, monospace;
}

.chat-input-height-handle {
  position: relative;
  width: 100%;
  height: 2px;
  cursor: ns-resize;
  background-color: #ffffff;
}
</style>

<style>
/* 数学公式样式 */
.math-inline {
  display: inline;
  vertical-align: middle;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace;
}

.math-block {
  display: block;
  text-align: center;
  margin: 1rem 0;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace;
  ;
}

/* Mermaid图表样式 */
.mermaid {
  background-color: white;
  border-radius: 8px;
  padding: 16px;
  margin: 1em 0;
  text-align: center;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace;
}

.dark .mermaid {
  background-color: #2d2d2d;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace;
}

/* 代码块样式 */
.hljs {
  position: relative;
  border-radius: 6px;
  padding: 1.5em 1em !important;
  font-size: 17px;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace;
  background: #f6f8fa !important;
  border: 1px solid #e1e4e8;
  margin: 1em 0;
}

.hljs code {
  font-family: inherit !important;
}

/* 代码块复制按钮 */
.hljs::before {
  position: absolute;
  right: 10px;
  top: 10px;
  padding: 2px 8px;
  font-size: 12px;
  background: #e1e4e8;
  border-radius: 4px;
  cursor: pointer;
  opacity: 0;
  transition: opacity 0.2s;
}

.hljs:hover::before {
  opacity: 1;
}

/* 暗色模式代码块 */
.dark .hljs {
  background: #2d2d2d !important;
  border-color: #444;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace !important;
}

.dark .hljs code {
  font-family: inherit !important;
}

.dark .hljs::before {
  background: #444;
  color: #fff;
}

.copy-button {
  opacity: 1 !important;
  background: rgba(255, 255, 255, 0.8);
  border: 1px solid rgba(0, 0, 0, 0.1);
}

/* 暗色模式适配 */
.dark .copy-button {
  background: rgba(60, 60, 60, 0.8) !important;
  border-color: rgba(255, 255, 255, 0.1) !important;
}

table {
  width: 100%;
  border-collapse: collapse;
  border: 1px solid #ddd;
}

th,
td {
  border: 1px solid #ddd;
  padding: 8px;
  text-align: center;
}

th {
  background-color: #f4f4f4;
  font-weight: bold;
}

tr:nth-child(even) {
  background-color: #f9f9f9;
}

img {
  max-width: 100%;
  height: auto;
  display: block;
  margin: 0 auto;
}
</style>