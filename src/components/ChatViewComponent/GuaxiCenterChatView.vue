<template>
  <div class="chat-container">
    <div class="chat-header">
      <n-avatar :src="activeChat?.ModelAvatar" size="medium" round />
      <div class="chat-header-title">
        <n-input v-if="isEditingTitle" size="small" v-model:value="editingTitle" class="chat-title-input"
          @keydown.enter.prevent="commitTitle" @blur="commitTitle" />
        <span v-else class="chat-title" @dblclick="beginEditTitle">{{ displayTitle }}</span>
      </div>
    </div>

    <div class="message-list" ref="messageContainer" @scroll="updateIsAtBottom">
      <div v-for="(message, index) in activeChatMessages.topic" :key="message.msgid" class="message-item"
        :class="message.sender === 'me' ? 'message-right' : 'message-left'">
        <n-avatar v-if="message.sender === 'me'" :src="activeChat.MeAvatar" size="medium" round />
        <n-avatar v-else :src="activeChat.ModelAvatar" size="medium" round />
        <div class="message-bubble">
          <div v-if="message.isPlaceholder" class="message-content" :ref="el => setMessageRef(el, index)">
            <span class="typing-dots"><i></i><i></i><i></i></span>
          </div>
          <div v-else class="message-content" v-html="message.content" :ref="el => setMessageRef(el, index)"></div>

          <div v-if="!message.isPlaceholder && message.sender !== 'me'" class="message-actions">
            <button class="message-action-btn" type="button" title="复制" @click="copyMessage(message)">
              <n-icon>
                <svg viewBox="0 0 24 24" width="16" height="16" xmlns="http://www.w3.org/2000/svg">
                  <path
                    d="M16 1H4c-1.1 0-2 .9-2 2v14h2V3h12V1zm4 4H8c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h12c1.1 0 2-.9 2-2V7c0-1.1-.9-2-2-2zm0 16H8V7h12v14z"
                    fill="currentColor" />
                </svg>
              </n-icon>
            </button>

            <button class="message-action-btn" type="button" title="分享" @click="shareMessage(message)">
              <n-icon>
                <svg viewBox="0 0 24 24" width="16" height="16" xmlns="http://www.w3.org/2000/svg">
                  <path
                    d="M18 16.08c-.76 0-1.44.3-1.96.77L8.91 12.7a2.5 2.5 0 0 0 0-1.39l7.02-4.11A2.99 2.99 0 1 0 14 5a2.9 2.9 0 0 0 .05.52L7.03 9.63a3 3 0 1 0 0 4.74l7.02 4.11c-.03.17-.05.34-.05.52a3 3 0 1 0 3-2.92z"
                    fill="currentColor" />
                </svg>
              </n-icon>
            </button>

            <button class="message-action-btn" type="button" title="重新生成" :disabled="isGenerating"
              @click="regenerateFrom(index)">
              <n-icon>
                <svg viewBox="0 0 24 24" width="16" height="16" xmlns="http://www.w3.org/2000/svg">
                  <path d="M17.65 6.35A7.95 7.95 0 0 0 12 4V1L7 6l5 5V7a6 6 0 1 1-6 6H4a8 8 0 1 0 13.65-6.65z"
                    fill="currentColor" />
                </svg>
              </n-icon>
            </button>
          </div>
        </div>
      </div>

      <div v-show="showJumpToBottom" class="jump-to-bottom" @click="jumpToBottom">
        <n-spin v-if="isGenerating" size="small" />
        <n-icon v-else>
          <svg viewBox="0 0 24 24" width="18" height="18" xmlns="http://www.w3.org/2000/svg">
            <path d="M12 16.5l-6-6 1.4-1.4L12 13.7l4.6-4.6L18 10.5l-6 6z" fill="currentColor" />
          </svg>
        </n-icon>
      </div>

    </div>

    <div class="input-area">
      <div class="chat-input-height-handle" @mousedown="startResize"></div>
      <n-input type="textarea" placeholder="请输入您的问题..." v-model:value="messageInput" :disabled="isGenerating"
        @keydown="handleKeydown" style="flex-grow: 1; height: 100%; padding: 2px; border-radius: 5px; box-sizing: border-box;   font-size: 17px;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace;" />
      <n-button @click="sendMessage" type="primary" :disabled="isGenerating"
        style="position: absolute; right: 10px; bottom: 8px; border-radius: 50%; padding: 0 20px; height: 36px; min-width: 36px; font-size: 12px;">
        <template #icon>
          <n-icon>
            <svg t="1737646310254" class="icon" viewBox="0 0 1178 1024" version="1.1" xmlns="http://www.w3.org/2000/svg"
              p-id="5954" width="48" height="48">
              <path
                d="M1145.732473 24.529036A96.502796 96.502796 0 0 0 1047.167948 6.302655L62.5449 378.38417a96.762677 96.762677 0 0 0-8.229592 177.44733l153.746105 75.175158a32.918368 32.918368 0 0 1 8.229592 5.804029c15.974071 41.130635 79.05606 202.292036 107.123301 260.540222a120.758435 120.758435 0 0 0 64.520002 59.530271c-1.611268-0.155929-3.465091-0.485113-5.318915-0.814297a88.359831 88.359831 0 0 0 14.518733 4.192761 96.190937 96.190937 0 0 0 87.59751-26.455973l57.260636-57.260636a32.485232 32.485232 0 0 1 38.722396-5.318915l246.662532 140.024344a97.143838 97.143838 0 0 0 47.748959 12.751536 97.022559 97.022559 0 0 0 94.042581-73.56389l205.999684-830.478457a95.671174 95.671174 0 0 0-29.453277-95.290014z m-868.005397 592.3747L805.56445 314.903695l-297.997861 305.222576a48.112794 48.112794 0 0 0-10.811085 17.325457c-0.155929 0.329184-0.155929 0.485113-0.329184 0.814297-0.641042 1.940451-59.53027 175.680134-94.371764 254.233756a58.404116 58.404116 0 0 1-20.651945-22.748325c-25.814931-54.211355-85.986243-207.628277-103.727511-252.795743z"
                fill="currentColor" p-id="5955"></path>
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

const displayTitle = computed(() => {
  const t = (activeChatMessages.value?.title ?? '').toString().trim();
  if (t) return t;
  return activeChat.value?.name || '未选择会话';
});

const isEditingTitle = ref(false);
const editingTitle = ref('');

const beginEditTitle = () => {
  if (!activeChat.value) return;
  const current = (activeChatMessages.value?.title ?? '').toString().trim();
  editingTitle.value = current || '';
  isEditingTitle.value = true;
  nextTick(() => {
    const el = document.querySelector('.chat-title-input input');
    if (el && typeof el.focus === 'function') {
      el.focus();
      if (typeof el.select === 'function') el.select();
    }
  });
};

const commitTitle = () => {
  if (!isEditingTitle.value) return;
  isEditingTitle.value = false;

  const nextTitle = (editingTitle.value ?? '').toString().trim();
  if (!activeChatMessages.value) return;

  if (nextTitle) {
    activeChatMessages.value.title = nextTitle;
    activeChatMessages.value.__titleLocked = true;
  } else {
    activeChatMessages.value.title = '';
    activeChatMessages.value.__titleLocked = false;
  }

  scheduleAutosave();
};

const pickAutoTitle = (raw) => {
  let s = (raw ?? '').toString();
  if (!s.trim()) return '';

  s = s.replace(/```[\s\S]*?```/g, ' ');
  s = s.replace(/`[^`]*`/g, ' ');
  s = s.replace(/\[\s*\S*?\]/g, ' ');

  const lines = s.split(/\r?\n/).map((x) => x.trim()).filter(Boolean);
  let first = lines[0] || '';

  first = first.replace(/^#{1,6}\s+/, '');
  first = first.replace(/^>\s+/, '');
  first = first.replace(/^[-*+]\s+/, '');

  const sentence = first.split(/([。！？.!?])\s/)[0] || first;
  const title = sentence.replace(/\s+/g, ' ').trim();

  const MAX = 24;
  if (title.length <= MAX) return title;
  return title.slice(0, MAX).trimEnd() + '…';
};

const streamTitleBuffer = new Map();

const maybeAutoSetTitle = (sessionId) => {
  const topic = activeChatMessages.value;
  if (!topic) return;
  if (topic.__titleLocked) return;

  const current = (topic.title ?? '').toString().trim();
  if (current) return;

  const buf = streamTitleBuffer.get(sessionId) || '';
  const nextTitle = pickAutoTitle(buf);
  if (!nextTitle) return;

  topic.title = nextTitle;
  topic.__titleLocked = false;
};

const autosaveTimer = ref(null);

const isWebModel = computed(() => chatStore.selectedModel === 7 || chatStore.selectedModel === 8);

const isGenerating = ref(false);
const waitingFirstToken = ref(false);
const generatingTimer = ref(null);
const GENERATING_IDLE_MS = 2500;

const stopGenerating = () => {
  isGenerating.value = false;
  waitingFirstToken.value = false;
  if (generatingTimer.value) {
    clearTimeout(generatingTimer.value);
    generatingTimer.value = null;
  }
};

const startGenerating = () => {
  isGenerating.value = true;
  waitingFirstToken.value = true;
  if (generatingTimer.value) {
    clearTimeout(generatingTimer.value);
    generatingTimer.value = null;
  }
};

const bumpGenerating = () => {
  isGenerating.value = true;
  waitingFirstToken.value = false;
  if (generatingTimer.value) {
    clearTimeout(generatingTimer.value);
  }
  generatingTimer.value = setTimeout(() => {
    generatingTimer.value = null;
    isGenerating.value = false;
    waitingFirstToken.value = false;
  }, GENERATING_IDLE_MS);
};

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
    const topicTitle = (activeChatMessages.value?.title ?? '').toString().trim() || null;
    await invoke('save_chat_session', {
      request: {
        model_id: chatStore.selectedModel,
        model_name: activeChat.value?.name || '',
        topic_id: chatStore.selectedTopic,
        session_id: sid,
        topic_title: topicTitle,
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

const normalizeMathDelimiters = (text) => {
  return (text ?? '').toString()
    .replace(/\\\[/g, '$$')
    .replace(/\\\]/g, '$$')
    .replace(/\\\(/g, '$')
    .replace(/\\\)/g, '$');
};

const isAtBottom = ref(true);
const showJumpToBottom = computed(() => !isAtBottom.value);

const messageContainer = ref(null);

const updateIsAtBottom = () => {
  const el = messageContainer.value;
  if (!el) return;
  const threshold = 40;
  isAtBottom.value = el.scrollHeight - el.scrollTop - el.clientHeight <= threshold;
};

const scrollToBottom = () => {
  nextTick(() => {
    const el = messageContainer.value;
    if (!el) return;
    el.scrollTop = el.scrollHeight;
    updateIsAtBottom();
  });
};

const jumpToBottom = () => {
  scrollToBottom();
};

const stripHtml = (html) => {
  const s = (html ?? '').toString();
  if (!s) return '';
  const div = document.createElement('div');
  div.innerHTML = s.replace(/<br\s*\/?\s*>/gi, '\n');
  return (div.textContent || div.innerText || '').toString();
};

const messageToText = (message) => {
  if (!message) return '';
  const raw = (message.raw ?? '').toString();
  if (raw.trim()) return raw;
  return stripHtml(message.content);
};

const writeClipboard = async (text) => {
  const value = (text ?? '').toString();
  if (!value) return;

  if (navigator?.clipboard?.writeText) {
    try {
      await navigator.clipboard.writeText(value);
      return;
    } catch (e) { }
  }

  try {
    const mod = await import('@tauri-apps/plugin-clipboard-manager');
    await mod.writeText(value);
  } catch (e) {
    console.error('写入剪贴板失败:', e);
  }
};

const copyMessage = async (message) => {
  await writeClipboard(messageToText(message));
};

const shareMessage = async (message) => {
  const text = messageToText(message);
  if (!text) return;

  const api = navigator?.share;
  if (typeof api === 'function') {
    try {
      await api({ text });
      return;
    } catch (e) { }
  }

  await writeClipboard(text);
};

const regenerateFrom = async (assistantIndex) => {
  if (isGenerating.value || !activeChat.value) return;
  const topic = Array.isArray(activeChatMessages.value?.topic) ? activeChatMessages.value.topic : [];

  let question = '';
  for (let i = assistantIndex - 1; i >= 0; i--) {
    const m = topic[i];
    if (m && m.sender === 'me') {
      question = messageToText(m);
      break;
    }
  }
  if (!question.trim()) return;

  const isWeb = isWebModel.value;
  const sessionId = ensureTopicSessionId(topic);

  startGenerating();

  topic.push({
    msgid: topic.length + 1,
    sender: activeChat.value.name,
    chatSessionId: sessionId,
    raw: '',
    content: '',
    isPlaceholder: true,
  });

  scrollToBottom();

  if (isWeb) {
    let provider = '';
    if (chatStore.selectedModel === 7) provider = 'yuanbao';
    if (chatStore.selectedModel === 8) provider = 'chatgpt';

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
      console.error('重新生成（网页模式）失败:', e);
      stopGenerating();
    }

    scheduleAutosave();
    return;
  }

  chatStore.setStreamTarget(chatStore.selectedModel, chatStore.selectedTopic);
  chatStore.accumulatedMessage = '';

  try {
    await askDeepSeek(question, sessionId);
  } catch (e) {
    console.error('重新生成失败:', e);
    stopGenerating();
  }

  scheduleAutosave();
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

  startGenerating();

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
    content: question
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;')
      .replace(/'/g, '&#39;')
      .replace(/\r?\n/g, '<br/>'),
  };

  topic.push(myMsg);

  if (!isWeb) {
    topic.push({
      msgid: topic.length + 1,
      sender: activeChat.value.name,
      chatSessionId: sessionId,
      raw: '',
      content: '',
      isPlaceholder: true,
    });
  }

  console.log('topic', topic);

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
      stopGenerating();
      return;
    }

    scrollToBottom();
    scheduleAutosave();
    return;
  }

  chatStore.setStreamTarget(chatStore.selectedModel, chatStore.selectedTopic);
  chatStore.accumulatedMessage = '';

  try {
    await askDeepSeek(question, sessionId);
  } catch (error) {
    console.error('Error during askDeepSeek:', error);
    stopGenerating();
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
  nextTick(() => {
    updateIsAtBottom();
  });

  listen("modle-streaming-respond-output", (event) => {
    const shouldFollow = isAtBottom.value;
    bumpGenerating();
    const rawPayload = typeof event.payload === 'string' ? event.payload : '';
    const payload = typeof event.payload === 'string' ? normalizeMathDelimiters(event.payload) : event.payload;
    if (rawPayload && (rawPayload.includes('$') || rawPayload.includes('`') || rawPayload.includes('```') || rawPayload.includes('\\[') || rawPayload.includes('\\]') || rawPayload.includes('\\(') || rawPayload.includes('\\)'))) {
      console.info('[modle-streaming-respond-output raw]', rawPayload.slice(0, 400));
    }

    if (typeof payload === 'string' && payload && (payload.includes('$') || payload.includes('`') || payload.includes('```'))) {
      console.info('[modle-streaming-respond-output normalized]', payload.slice(0, 400));
    }

    chatStore.collectChatSessionStream(payload);

    const topic = Array.isArray(activeChatMessages.value?.topic) ? activeChatMessages.value.topic : [];
    const latest = topic[topic.length - 1];
    if (latest && latest.sender !== 'me') {
      if (!latest.chatSessionId) {
        latest.chatSessionId = chatStore.sendChatSessionId || '';
      }
      if (latest.isPlaceholder && typeof payload === 'string' && payload.length > 0) {
        latest.isPlaceholder = false;
      }

      const sid = (latest.chatSessionId || '').toString().trim();
      if (sid && typeof payload === 'string' && payload) {
        const prev = streamTitleBuffer.get(sid) || '';
        const next = (prev + payload).slice(0, 800);
        streamTitleBuffer.set(sid, next);
        maybeAutoSetTitle(sid);
      }
    }

    if (shouldFollow) {
      scrollToBottom();
    } else {
      nextTick(updateIsAtBottom);
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
    const shouldFollow = isAtBottom.value;
    const payload = event.payload || {};
    const modelId = payload.model_id;
    const topicId = payload.topic_id;
    const sessionId = payload.session_id;
    const title = payload.title;
    const messages = payload.messages;

    const normalizedMessages = Array.isArray(messages)
      ? messages.map((m) => ({
        ...m,
        content: normalizeMathDelimiters(m?.content),
      }))
      : messages;

    chatStore.appendMessagesToTopic({
      modelId,
      topicId,
      sessionId,
      title,
      messages: normalizedMessages,
    });

    if (modelId === chatStore.selectedModel && topicId === chatStore.selectedTopic) {
      bumpGenerating();
      if (shouldFollow) {
        scrollToBottom();
      } else {
        nextTick(updateIsAtBottom);
      }
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
  stopGenerating();
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
  border: 1px solid var(--guaxi-border);
  display: flex;
  flex-direction: column;
  padding: 0;
  box-sizing: border-box;
  min-height: 0;
  background: var(--guaxi-content-bg);
}

.chat-header {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 14px;
  border-bottom: 1px solid var(--guaxi-border);
  background: var(--guaxi-panel-bg);
}

.chat-header-title {
  display: flex;
  align-items: center;
  min-width: 0;
  flex: 1 1 auto;
}

.chat-title {
  font-size: 14px;
  font-weight: 650;
  color: var(--guaxi-text-primary);
  cursor: text;
  user-select: text;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.chat-title-input {
  max-width: 420px;
}

.chat-title-input :deep(.n-input-wrapper) {
  background-color: var(--guaxi-panel-bg) !important;
}

.chat-title-input :deep(.n-input__border),
.chat-title-input :deep(.n-input__state-border) {
  border-color: var(--guaxi-border) !important;
}

.chat-title-input :deep(.n-input__input-el) {
  color: var(--guaxi-text-primary) !important;
}

.message-list {
  flex: 1 1 auto;
  overflow-y: auto;
  padding: 16px;
  background: radial-gradient(1200px 600px at 20% 0%, rgba(45, 212, 191, 0.10) 0%, rgba(45, 212, 191, 0) 60%),
    radial-gradient(1000px 500px at 90% 10%, rgba(99, 102, 241, 0.08) 0%, rgba(99, 102, 241, 0) 55%),
    var(--guaxi-content-bg);
  min-height: 0;
  position: relative;
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
  min-width: 0;
  padding: 10px 12px;
  border-radius: 16px;
  margin: 0 6px;
  box-shadow: 0 8px 22px rgba(15, 23, 42, 0.06);
  border: 1px solid var(--guaxi-border);
  box-sizing: border-box;
}

.message-content {
  min-width: 0;
}

.message-left .message-bubble {
  background: var(--guaxi-panel-bg);
  font-size: 16px;
  font-family: 'Maple Mono NF CN', 'Noto Sans CJK SC', Consolas, 'Courier New', Courier, monospace;
  border-top-left-radius: 6px;
}

.message-right .message-bubble {
  background: linear-gradient(135deg, #2dd4bf 0%, #22c55e 100%);
  font-size: 16px;
  font-family: 'Maple Mono NF CN', 'Noto Sans CJK SC', Consolas, 'Courier New', Courier, monospace;
  color: rgba(255, 255, 255, 0.98);
  border-top-right-radius: 6px;
}

.input-area {
  flex: 0 0 auto;
  border-top: 1px solid var(--guaxi-border);
  position: relative;
  background-color: var(--guaxi-panel-bg);
  padding-bottom: 2px;
  height: 80px;
  min-height: 80px;
  font-size: 17px;
  font-family: 'Maple Mono NF CN', 'Noto Sans CJK SC', Consolas, 'Courier New', Courier, monospace;
}

.input-area :deep(.n-input) {
  flex: 1 1 auto;
  height: calc(100% - 2px);
}

.input-area :deep(.n-input .n-input-wrapper) {
  background-color: var(--guaxi-content-bg) !important;
}

.input-area :deep(.n-input .n-input__border),
.input-area :deep(.n-input .n-input__state-border) {
  border-color: var(--guaxi-border) !important;
}

.input-area :deep(.n-input__textarea-el) {
  height: 100% !important;
  background-color: var(--guaxi-content-bg) !important;
  color: var(--guaxi-text-primary) !important;
}

.input-area :deep(.n-input__textarea-el::placeholder) {
  color: var(--guaxi-text-secondary);
  opacity: 0.85;
}

.jump-to-bottom {
  position: absolute;
  right: 14px;
  bottom: 14px;
  width: 34px;
  height: 34px;
  border-radius: 999px;
  background: var(--guaxi-panel-bg);
  border: 1px solid var(--guaxi-border);
  box-shadow: 0 2px 10px rgba(0, 0, 0, 0.08);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  color: var(--guaxi-text-secondary);
  z-index: 3;
}

.typing-dots {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  height: 14px;
}

.typing-dots i {
  width: 6px;
  height: 6px;
  border-radius: 999px;
  background: rgba(0, 0, 0, 0.35);
  animation: typingDot 1.2s infinite ease-in-out;
}

.typing-dots i:nth-child(2) {
  animation-delay: 0.2s;
}

.typing-dots i:nth-child(3) {
  animation-delay: 0.4s;
}

@keyframes typingDot {

  0%,
  80%,
  100% {
    transform: translateY(0);
    opacity: 0.35;
  }

  40% {
    transform: translateY(-3px);
    opacity: 1;
  }
}

.message-actions {
  display: flex;
  gap: 10px;
  margin-top: 6px;
  opacity: 0;
  transition: opacity 120ms ease;
}

.message-left .message-actions {
  justify-content: flex-start;
}

.message-item:hover .message-actions {
  opacity: 1;
}

.message-action-btn {
  width: 26px;
  height: 26px;
  border-radius: 8px;
  border: 1px solid var(--guaxi-border);
  background: var(--guaxi-panel-bg);
  color: var(--guaxi-text-secondary);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
}

.message-action-btn:disabled {
  cursor: not-allowed;
  opacity: 0.55;
}

.chat-input-height-handle {
  position: relative;
  width: 100%;
  height: 2px;
  cursor: ns-resize;
  background-color: var(--guaxi-border);
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
  background-color: var(--guaxi-panel-bg);
  border-radius: 8px;
  padding: 16px;
  margin: 1em 0;
  text-align: center;
  border: 1px solid var(--guaxi-border);
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace;
}

:root[data-guaxi-theme='dark'] .mermaid {
  background-color: var(--guaxi-panel-bg);
}

/* 代码块样式 */
.hljs {
  position: relative;
  border-radius: 6px;
  padding: 1.5em 1em !important;
  font-size: 17px;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace;
  background: var(--guaxi-header-bg) !important;
  border: 1px solid var(--guaxi-border);
  margin: 1em 0;
  max-width: 100%;
  overflow-x: auto;
  box-sizing: border-box;
}

.hljs code {
  font-family: inherit !important;
  display: block;
  white-space: pre;
}

pre {
  max-width: 100%;
  overflow-x: auto;
  box-sizing: border-box;
}

pre code {
  display: block;
  white-space: pre;
}

/* 代码块复制按钮 */
.hljs::before {
  position: absolute;
  right: 10px;
  top: 10px;
  padding: 2px 8px;
  font-size: 12px;
  background: var(--guaxi-panel-bg);
  border: 1px solid var(--guaxi-border);
  border-radius: 4px;
  cursor: pointer;
  opacity: 0;
  transition: opacity 0.2s;
  color: var(--guaxi-text-secondary);
}

.hljs:hover::before {
  opacity: 1;
}

/* 暗色模式代码块 */
:root[data-guaxi-theme='dark'] .hljs {
  background: var(--guaxi-header-bg) !important;
  border-color: var(--guaxi-border);
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace !important;
}

.dark .hljs code {
  font-family: inherit !important;
}

:root[data-guaxi-theme='dark'] .hljs::before {
  background: var(--guaxi-panel-bg);
  color: var(--guaxi-text-secondary);
}

.copy-button {
  opacity: 1 !important;
  background: var(--guaxi-panel-bg);
  border: 1px solid var(--guaxi-border);
  color: var(--guaxi-text-secondary);
}

/* 暗色模式适配 */
.dark .copy-button {
  background: rgba(60, 60, 60, 0.8) !important;
  border-color: rgba(255, 255, 255, 0.1) !important;
}

table {
  width: 100%;
  border-collapse: collapse;
  border: 1px solid var(--guaxi-border);
}

th,
td {
  border: 1px solid var(--guaxi-border);
  padding: 8px;
  text-align: center;
}

th {
  background-color: var(--guaxi-header-bg);
  color: var(--guaxi-text-primary);
  font-weight: bold;
}

tr:nth-child(even) {
  background-color: var(--guaxi-panel-bg);
}

img {
  max-width: 100%;
  height: auto;
  display: block;
  margin: 0 auto;
}
</style>