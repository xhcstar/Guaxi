<template>
  <div class="chat-container">
    <div class="chat-header">
      <n-avatar :src="activeChat?.ModelAvatar" size="medium" round />
      <div class="chat-header-title">
        <n-input v-if="isEditingTitle" size="small" v-model:value="editingTitle" class="chat-title-input"
          @keydown.enter.prevent="commitTitle" @blur="commitTitle" />
        <span v-else class="chat-title" @dblclick="beginEditTitle">{{ displayTitle }}</span>
      </div>
      <button
        class="header-select-btn"
        :class="{ active: isSelectingMessages }"
        type="button"
        :title="isSelectingMessages ? '退出多选' : '进入多选'"
        :disabled="isGenerating"
        @click="toggleSelectMode"
      >
        <svg viewBox="0 0 24 24" width="18" height="18" xmlns="http://www.w3.org/2000/svg">
          <path d="M9 11l3 3L22 4l-1.4-1.4L12 11.2 10.4 9.6zM2 6h8V4H2zm0 6h5v-2H2zm0 6h8v-2H2z" fill="currentColor" />
        </svg>
      </button>
    </div>
    <div class="message-list" ref="messageContainer" @scroll="updateIsAtBottom">
      <div v-if="isSelectingMessages" class="select-toolbar">
        <span class="select-toolbar-text">已选 {{ selectedMessageIds.size }} 条</span>
        <button class="select-toolbar-btn" type="button" @click="selectAllVisibleMessages">全选可见</button>
        <button class="select-toolbar-btn" type="button" @click="clearSelectedMessages">清空</button>
        <button class="select-toolbar-btn primary" type="button" :disabled="selectedMessageIds.size === 0" @click="sendSelectedMessagesToEditor">发送到编辑器</button>
      </div>
      <div v-if="displayedMessages.length === 0" class="message-empty">
        未找到匹配消息
      </div>
      <div v-for="(message, index) in displayedMessages" :key="message.msgid" class="message-item"
        :class="message.sender === 'me' ? 'message-right' : 'message-left'">
        <label v-if="isSelectingMessages" class="msg-select-box">
          <input
            type="checkbox"
            :checked="selectedMessageIds.has(String(message.msgid))"
            @change="toggleSelectMessage(message)"
          />
        </label>
        <n-avatar v-if="message.sender === 'me'" :src="activeChat.MeAvatar" size="medium" round />
        <n-avatar v-else :src="activeChat.ModelAvatar" size="medium" round />
        <div class="message-bubble" :class="{ 'message-bubble-selected': selectedMessageIds.has(String(message.msgid)) }">
          <div v-if="message.isPlaceholder" class="message-content" :ref="el => setMessageRef(el, index)">
            <span class="typing-dots"><i></i><i></i><i></i></span>
          </div>
          <div v-else class="message-content" v-html="message.content" :ref="el => setMessageRef(el, index)"></div>

          <div v-if="!message.isPlaceholder" class="message-actions">
            <button class="message-action-btn" type="button" :class="{ copied: copiedMessageId === String(message.msgid) }"
              :title="copiedMessageId === String(message.msgid) ? '已复制' : '复制'" @click="copyMessage(message)">
              <n-icon>
                <svg v-if="copiedMessageId === String(message.msgid)" viewBox="0 0 24 24" width="16" height="16"
                  xmlns="http://www.w3.org/2000/svg">
                  <path
                    d="M9.00002 16.2L4.80002 12L3.40002 13.4L9.00002 19L21 7.00001L19.6 5.60001L9.00002 16.2Z"
                    fill="currentColor" />
                </svg>
                <svg v-else viewBox="0 0 24 24" width="16" height="16" xmlns="http://www.w3.org/2000/svg">
                  <path
                    d="M16 1H4c-1.1 0-2 .9-2 2v14h2V3h12V1zm4 4H8c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h12c1.1 0 2-.9 2-2V7c0-1.1-.9-2-2-2zm0 16H8V7h12v14z"
                    fill="currentColor" />
                </svg>
              </n-icon>
            </button>
            <span v-if="copiedMessageId === String(message.msgid)" class="message-copy-hint">已复制</span>

            <button v-if="message.sender !== 'me'" class="message-action-btn" type="button" title="分享" @click="shareMessage(message)">
              <n-icon>
                <svg viewBox="0 0 24 24" width="16" height="16" xmlns="http://www.w3.org/2000/svg">
                  <path
                    d="M18 16.08c-.76 0-1.44.3-1.96.77L8.91 12.7a2.5 2.5 0 0 0 0-1.39l7.02-4.11A2.99 2.99 0 1 0 14 5a2.9 2.9 0 0 0 .05.52L7.03 9.63a3 3 0 1 0 0 4.74l7.02 4.11c-.03.17-.05.34-.05.52a3 3 0 1 0 3-2.92z"
                    fill="currentColor" />
                </svg>
              </n-icon>
            </button>

            <button v-if="message.sender !== 'me'" class="message-action-btn" type="button" title="重新生成" :disabled="isGenerating"
              @click="regenerateFrom(message)">
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
    </div>
    <div class="input-area">
      <div v-show="showJumpToBottom" class="jump-to-bottom" @click="jumpToBottom">
        <n-spin v-if="isGenerating" size="small" />
        <n-icon v-else>
          <svg viewBox="0 0 24 24" width="18" height="18" xmlns="http://www.w3.org/2000/svg">
            <path d="M12 16.5l-6-6 1.4-1.4L12 13.7l4.6-4.6L18 10.5l-6 6z" fill="currentColor" />
          </svg>
        </n-icon>
      </div>
      <div class="composer-shell" @click="focusComposerInput">
        <div v-if="pendingAttachments.length" class="attachment-list">
          <div v-for="file in pendingAttachments" :key="file.id" class="attachment-chip">
            <span class="attachment-name" :title="file.path">{{ file.name }}</span>
            <span class="attachment-size">{{ formatAttachmentSize(file.size) }}</span>
            <button class="attachment-remove" type="button" title="移除附件" @click="removeAttachment(file.id)">×</button>
          </div>
        </div>

        <textarea
          ref="composerInputRef"
          class="composer-input"
          :placeholder="isTranslateMode ? `翻译模式：将发送后返回${getTranslateLangLabel(translateToLang.value)}译文` : '给 Guaxi 发送消息...'"
          v-model="messageInput"
          :disabled="isGenerating"
          @keydown="handleKeydown"
        />

        <div class="composer-toolbar">
          <button
            class="composer-icon-btn"
            type="button"
            title="添加附件"
            :disabled="isGenerating"
            @click="pickAttachments"
          >
            <svg viewBox="0 0 24 24" width="18" height="18" xmlns="http://www.w3.org/2000/svg">
              <path d="M17.5 6.5v9a5.5 5.5 0 0 1-11 0v-10a4 4 0 0 1 8 0v9a2.5 2.5 0 1 1-5 0V8h1.8v6.5a.7.7 0 1 0 1.4 0v-9a2.2 2.2 0 0 0-4.4 0v10a3.7 3.7 0 0 0 7.4 0v-9h1.8z" fill="currentColor" />
            </svg>
          </button>

          <div class="translate-group">
          <button
            class="composer-icon-btn"
            :class="{ active: isTranslateMode }"
            type="button"
            :title="isTranslateMode ? '退出翻译模式' : '进入翻译模式'"
            :disabled="isGenerating"
            @click="toggleTranslateMode"
          >
              <svg viewBox="0 0 24 24" width="18" height="18" xmlns="http://www.w3.org/2000/svg">
                <path d="M12.87 15.07l-2.54-2.55.03-.03A18.4 18.4 0 0 0 13.07 7h2V5h-5V3H8v2H3v2h8.17a16.2 16.2 0 0 1-2.39 3.5A15.9 15.9 0 0 1 7.26 8H5.24a18 18 0 0 0 2.25 3.53l-5.09 5.1L3.82 18l5.05-5.05 3.15 3.15.85-1.03zM17.5 10h-2l-4.5 12h2l1.12-3h4.76l1.12 3h2l-4.5-12zm-2.49 7l1.87-5 1.87 5h-3.74z" fill="currentColor" />
              </svg>
            </button>
            <select class="translate-select" v-model="translateToLang" :disabled="isGenerating">
              <option v-for="opt in translateLangOptions" :key="opt.value" :value="opt.value">{{ opt.label }}</option>
            </select>
          </div>
          <div class="composer-spacer"></div>

          <button
            class="send-btn"
            type="button"
            :disabled="!canClickSend"
            @click="handleSendButtonClick"
            :title="isGenerating ? '停止生成' : '发送'"
          >
            <svg
              v-if="isGenerating"
              viewBox="0 0 24 24"
              width="18"
              height="18"
              xmlns="http://www.w3.org/2000/svg"
            >
              <path d="M7 7h10v10H7z" fill="currentColor" />
            </svg>
            <svg v-else viewBox="0 0 24 24" width="18" height="18" xmlns="http://www.w3.org/2000/svg">
              <path d="M12 4l-7 7 1.4 1.4 4.6-4.6V20h2V7.8l4.6 4.6L19 11z" fill="currentColor" />
            </svg>
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { createLogger } from '../../utils/logger'
import { ref, nextTick, computed, onMounted, onUnmounted, watch } from 'vue'
import { useChatStore } from "../../stores/chatStore";
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { addCopyButtons, addMediaViewerButtons, renderMarkdown } from "../GuaxiMarkdownRender/GuaxiMarker";
import mermaid from 'mermaid';

const logger = createLogger('GuaxiCenterChatView')
const chatStore = useChatStore();
const messageInput = ref("");
const inputHistory = ref([]);
const historyIndex = ref(-1);
const messageRefs = ref([]);
const composerInputRef = ref(null);
const isSelectingMessages = ref(false);
const selectedMessageIds = ref(new Set());
const isTranslateMode = ref(false);
const translateToLang = ref('en');
const translateLangOptions = [
  { value: 'en', label: '英语' },
  { value: 'zh-CN', label: '简体中文' },
  { value: 'ja', label: '日语' },
  { value: 'ko', label: '韩语' },
  { value: 'fr', label: '法语' },
  { value: 'de', label: '德语' },
  { value: 'ru', label: '俄语' },
  { value: 'es', label: '西班牙语' },
];
const pendingAttachments = ref([]);


// 当前激活的聊天会话
const activeChat = computed(() =>
  chatStore.SelectModelchatSessions.find((chatmodel) => chatmodel.id === chatStore.selectedModel)
);

// 当前会话的消息记录
const activeChatMessages = computed(() =>
  activeChat.value?.MessagesList.find((topic) => topic.topicid === chatStore.selectedTopic) || { topicid: chatStore.selectedTopic, topic: [] }
);

const displayedMessages = computed(() => {
  const list = Array.isArray(activeChatMessages.value?.topic) ? activeChatMessages.value.topic : [];
  const q = (chatStore.chatMessageSearchQuery ?? '').toString().trim().toLowerCase();
  if (!q) return list;
  return list.filter((m) => {
    const raw = (m?.raw ?? '').toString().toLowerCase();
    const htmlText = stripHtml(m?.content || '').toLowerCase();
    return raw.includes(q) || htmlText.includes(q);
  });
});

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

const autosaveTimer = ref(null);

const isWebModel = computed(() => chatStore.selectedModel === 7 || chatStore.selectedModel === 8);
const canClickSend = computed(() => isGenerating.value || messageInput.value.trim().length > 0)

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
      logger.error('预加载网页模型窗口失败:', e);
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
    logger.error('自动保存会话失败:', e);
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

const isAtBottom = ref(true);
const showJumpToBottom = computed(() => !isAtBottom.value);

const messageContainer = ref(null);
let rerenderSeq = 0;

const renderMermaidInContainer = async (container) => {
  if (!(container instanceof HTMLElement)) return;
  const nodes = Array.from(container.querySelectorAll('.mermaid') || []);
  if (!nodes.length) return;

  try {
    mermaid.initialize({ startOnLoad: false, theme: 'default', securityLevel: 'loose' });
  } catch (e) {
    logger.warn('Mermaid初始化失败:', e);
  }

  nodes.forEach((el) => {
    try {
      el.removeAttribute('data-processed');
      const content = String(el.textContent || '')
        .replace(/&gt;/g, '>')
        .replace(/&lt;/g, '<')
        .replace(/&amp;/g, '&');
      el.innerHTML = content;
    } catch (err) {
      logger.error('Mermaid内容处理失败:', err);
    }
  });

  await nextTick();
  requestAnimationFrame(() => {
    Promise.resolve(mermaid.run({ nodes }))
      .then(() => {
        logger.info('Mermaid渲染完成', { count: nodes.length });
      })
      .catch((err) => {
        logger.error('Mermaid渲染失败', err);
      });
  });
};

const rerenderActiveTopic = async () => {
  const token = ++rerenderSeq;
  const topic = Array.isArray(activeChatMessages.value?.topic) ? activeChatMessages.value.topic : [];
  if (!topic.length) return;

  let updated = 0;
  for (const msg of topic) {
    if (!msg || msg.sender === 'me') continue;
    const raw = (msg.raw ?? '').toString();
    if (!raw.trim()) continue;
    try {
      const html = await renderMarkdown(raw, {
        preserveOriginal: true,
        basePath: '',
        mermaid: true,
        math: true,
        codeHighlight: true,
        chatPreprocess: true,
      });
      if (token !== rerenderSeq) return;
      msg.content = String(html ?? '');
      updated += 1;
    } catch (e) {
      logger.error('重新渲染消息失败:', e);
    }
  }

  if (updated) {
    logger.info('切换会话后重渲染消息', { count: updated });
  }

  await nextTick();
  const container = messageContainer.value;
  if (container) {
    const contentNodes = Array.from(container.querySelectorAll('.message-content') || []);
    contentNodes.forEach((node) => {
      addCopyButtons(node);
      addMediaViewerButtons(node);
    });
    await renderMermaidInContainer(container);
  }
};

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

const formatAttachmentSize = (size) => {
  const n = Number(size);
  if (!Number.isFinite(n) || n <= 0) return '';
  if (n < 1024) return `${n}B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)}KB`;
  return `${(n / (1024 * 1024)).toFixed(1)}MB`;
};

const parseAttachmentName = (path) => {
  const p = (path ?? '').toString();
  if (!p) return '';
  const i = Math.max(p.lastIndexOf('/'), p.lastIndexOf('\\'));
  return i >= 0 ? p.slice(i + 1) : p;
};

const pickAttachments = async () => {
  try {
    const picked = await open({ multiple: true, directory: false });
    const paths = Array.isArray(picked) ? picked : (typeof picked === 'string' ? [picked] : []);
    if (!paths.length) return;

    const exists = new Set(pendingAttachments.value.map((x) => x.path));
    for (const p of paths) {
      const path = (p ?? '').toString();
      if (!path || exists.has(path)) continue;
      exists.add(path);
      pendingAttachments.value.push({
        id: `${Date.now()}-${Math.random().toString(16).slice(2)}`,
        path,
        name: parseAttachmentName(path),
        size: 0,
      });
    }
  } catch (e) {
    logger.error('选择附件失败:', e);
  }
};

const removeAttachment = (id) => {
  pendingAttachments.value = pendingAttachments.value.filter((x) => x.id !== id);
};

const getTranslateLangLabel = (value) => {
  const v = (value ?? '').toString();
  const hit = translateLangOptions.find((x) => x.value === v);
  return hit?.label || v || '目标语言';
};

const getFirstUserMessageText = (topic) => {
  const list = Array.isArray(topic) ? topic : [];
  const first = list.find((m) => m?.sender === 'me');
  if (!first) return '';
  return messageToText(first).trim();
};

const updateTopicSummaryTitle = () => {
  const topicMeta = activeChatMessages.value;
  if (!topicMeta || topicMeta.__titleLocked) return;

  const list = Array.isArray(topicMeta?.topic) ? topicMeta.topic : [];
  const firstUser = getFirstUserMessageText(list);
  const latestAssistant = [...list].reverse().find((m) => m?.sender !== 'me' && !m?.isPlaceholder);
  const basis = (latestAssistant?.raw ?? '').toString().trim() || firstUser;
  const nextTitle = pickAutoTitle(basis);
  if (!nextTitle) return;

  if ((topicMeta.title ?? '').toString().trim() !== nextTitle) {
    topicMeta.title = nextTitle;
    topicMeta.__titleLocked = false;
  }
};

const toggleTranslateMode = () => {
  isTranslateMode.value = !isTranslateMode.value;
};

const toggleSelectMode = () => {
  isSelectingMessages.value = !isSelectingMessages.value;
  if (!isSelectingMessages.value) {
    selectedMessageIds.value = new Set();
  }
};

const toggleSelectMessage = (message) => {
  const id = String(message?.msgid ?? '');
  if (!id) return;
  const next = new Set(selectedMessageIds.value);
  if (next.has(id)) next.delete(id);
  else next.add(id);
  selectedMessageIds.value = next;
};

const clearSelectedMessages = () => {
  selectedMessageIds.value = new Set();
};

const selectAllVisibleMessages = () => {
  const next = new Set();
  for (const m of displayedMessages.value) {
    const id = String(m?.msgid ?? '');
    if (id) next.add(id);
  }
  selectedMessageIds.value = next;
};

const sendSelectedMessagesToEditor = async () => {
  const selected = [];
  const all = Array.isArray(activeChatMessages.value?.topic) ? activeChatMessages.value.topic : [];
  for (const m of all) {
    const id = String(m?.msgid ?? '');
    if (!selectedMessageIds.value.has(id)) continue;
    const text = messageToText(m);
    if (!text.trim()) continue;
    selected.push(text.trim());
  }
  if (!selected.length) return;

  const content = selected.join('\n\n');
  try {
    const pendingKey = 'guaxi.pending_template_insert';
    localStorage.setItem(pendingKey, content);
    if (window.location.hash !== '#/') {
      window.location.hash = '/';
    }
    await new Promise((resolve) => setTimeout(resolve, 220));
    await invoke('emit_template_insert', { content });
    isSelectingMessages.value = false;
    selectedMessageIds.value = new Set();
  } catch (e) {
    logger.error('发送选中消息到编辑器失败:', e);
  }
};

const focusComposerInput = () => {
  if (isGenerating.value) return;
  const el = composerInputRef.value;
  if (el && typeof el.focus === 'function') {
    el.focus();
  }
};

const buildQuestionWithAttachments = (question) => {
  const text = (question ?? '').toString().trim();
  const files = Array.isArray(pendingAttachments.value) ? pendingAttachments.value : [];
  if (!files.length) return text;
  const list = files.map((f, idx) => `${idx + 1}. ${f.name} (${f.path})`).join('\n');
  return `${text}\n\n[附件]\n${list}\n[/附件]`;
};

const buildTranslatePrompt = (text) => {
  const target = getTranslateLangLabel(translateToLang.value);
  return [
    `请将下面文本准确翻译为${target}。`,
    '只输出译文，不要解释，不要添加额外内容。',
    '',
    text,
  ].join('\n');
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

const copiedMessageId = ref('');
let copyHintTimer = null;

watch(
  [() => chatStore.selectedModel, () => chatStore.selectedTopic],
  async () => {
    isSelectingMessages.value = false;
    selectedMessageIds.value = new Set();
    await nextTick();
    rerenderActiveTopic();
  },
  { immediate: true }
);

watch(
  () => {
    const topic = Array.isArray(activeChatMessages.value?.topic) ? activeChatMessages.value.topic : [];
    const latest = topic[topic.length - 1];
    return {
      modelId: chatStore.selectedModel,
      topicId: chatStore.selectedTopic,
      len: topic.length,
      sender: latest?.sender || '',
      raw: (latest?.raw ?? '').toString(),
      content: (latest?.content ?? '').toString(),
      isPlaceholder: !!latest?.isPlaceholder,
    };
  },
  () => {
    const topic = Array.isArray(activeChatMessages.value?.topic) ? activeChatMessages.value.topic : [];
    const latest = topic[topic.length - 1];
    if (!latest || latest.sender === 'me') return;

    const raw = (latest.raw ?? '').toString();
    const content = (latest.content ?? '').toString();
    if (latest.isPlaceholder && (raw || content)) {
      latest.isPlaceholder = false;
    }

    if (raw || content) {
      bumpGenerating();
      updateTopicSummaryTitle();
      const topicMeta = activeChatMessages.value;
      if (topicMeta && !topicMeta.__titleLocked) {
        const current = (topicMeta.title ?? '').toString().trim();
        if (!current) {
          const autoTitle = pickAutoTitle(raw || content);
          if (autoTitle) {
            topicMeta.title = autoTitle;
            topicMeta.__titleLocked = false;
          }
        }
      }
    }

    if (isAtBottom.value) {
      scrollToBottom();
    } else {
      nextTick(updateIsAtBottom);
    }

    scheduleAutosave();
  },
  { flush: 'post' }
);

const writeClipboard = async (text) => {
  const value = (text ?? '').toString();
  if (!value) return false;

  if (navigator?.clipboard?.writeText) {
    try {
      await navigator.clipboard.writeText(value);
      return true;
    } catch (e) { }
  }

  try {
    const mod = await import('@tauri-apps/plugin-clipboard-manager');
    await mod.writeText(value);
    return true;
  } catch (e) {
    logger.error('写入剪贴板失败:', e);
    return false;
  }
};

const copyMessage = async (message) => {
  const text = messageToText(message);
  const ok = await writeClipboard(text);
  if (!ok) return;

  const id = String(message?.msgid ?? '');
  copiedMessageId.value = id;
  logger.info('复制消息成功:', { msgid: id, len: text.length });

  if (copyHintTimer) {
    clearTimeout(copyHintTimer);
  }
  copyHintTimer = setTimeout(() => {
    if (copiedMessageId.value === id) copiedMessageId.value = '';
    copyHintTimer = null;
  }, 1200);
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

const regenerateFrom = async (assistantMessage) => {
  if (isGenerating.value || !activeChat.value) return;
  const topic = Array.isArray(activeChatMessages.value?.topic) ? activeChatMessages.value.topic : [];
  const assistantMsgId = String(assistantMessage?.msgid ?? '');
  const assistantIndex = topic.findIndex((m) => String(m?.msgid ?? '') === assistantMsgId);
  if (assistantIndex <= 0) return;

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
  chatStore.setStreamTarget(chatStore.selectedModel, chatStore.selectedTopic);

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
          attachments: [],
        },
      });
    } catch (e) {
      logger.error('重新生成（网页模式）失败:', e);
    } finally {
      stopGenerating();
      chatStore.clearStreamTarget();
    }

    scheduleAutosave();
    return;
  }

  chatStore.accumulatedMessage = '';

  try {
    await askDeepSeek(question, sessionId);
  } catch (e) {
    logger.error('重新生成失败:', e);
  } finally {
    stopGenerating();
    chatStore.clearStreamTarget();
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
    logger.info('Request:', request);
    // const response = await invoke('get_chat_answer', { request });
    const response = await invoke('get_chat_answer_stream', { request });
    logger.info('Response:', response);
    return response;
  } catch (error) {
    logger.error('Error calling API:', error);
  }
};

// 发送消息
const sendMessage = async (options = {}) => {
  if (!messageInput.value.trim() || !activeChat.value) return;

  const translateMode = typeof options.translateMode === 'boolean' ? options.translateMode : isTranslateMode.value;
  const isWeb = isWebModel.value;
  const attachmentsForSend = pendingAttachments.value.map((x) => x.path);

  const plainQuestion = messageInput.value.trim();
  let question = plainQuestion;
  if (translateMode) {
    question = buildTranslatePrompt(plainQuestion);
  } else {
    question = buildQuestionWithAttachments(plainQuestion);
  }

  startGenerating();

  inputHistory.value.push(plainQuestion);
  historyIndex.value = inputHistory.value.length;

  const topic = Array.isArray(activeChatMessages.value?.topic) ? activeChatMessages.value.topic : [];

  const sessionId = ensureTopicSessionId(topic);
  chatStore.setStreamTarget(chatStore.selectedModel, chatStore.selectedTopic);

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
    raw: plainQuestion,
    content: plainQuestion
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

  logger.info('topic', topic);

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
          attachments: attachmentsForSend,
        },
      });
    } catch (e) {
      logger.error('发送到网页模型失败:', e);
      return;
    } finally {
      stopGenerating();
      chatStore.clearStreamTarget();
    }

    pendingAttachments.value = [];
    scrollToBottom();
    scheduleAutosave();
    return;
  }

  chatStore.accumulatedMessage = '';

  try {
    await askDeepSeek(question, sessionId);
  } catch (error) {
    logger.error('Error during askDeepSeek:', error);
    return;
  } finally {
    stopGenerating();
    chatStore.clearStreamTarget();
  }

  pendingAttachments.value = [];
  scrollToBottom();
  scheduleAutosave();
};

const stopQuestion = async () => {
  if (!isGenerating.value) return
  try {
    await invoke('cancel_chat_stream')
  } catch (error) {
    logger.warn('停止生成失败:', error)
  }
  stopGenerating()
  chatStore.clearStreamTarget()
}

const handleSendButtonClick = () => {
  if (isGenerating.value) {
    stopQuestion()
    return
  }
  sendMessage()
}

const handleKeydown = (event) => {
  const resolveInputEl = () => {
    let el = event?.target;
    if (!el || typeof el.selectionStart !== 'number') {
      const current = event?.currentTarget;
      if (current && typeof current.querySelector === 'function') {
        const found = current.querySelector('textarea, input');
        if (found) el = found;
      }
    }
    return el;
  };

  if ((event.key === 'ArrowUp' || event.key === 'ArrowDown') && !event.shiftKey) {
    const el = resolveInputEl();
    const hasSelectionApi = el && typeof el.selectionStart === 'number' && typeof el.selectionEnd === 'number';
    if (hasSelectionApi) {
      const value = (el.value ?? '').toString();
      const isEmpty = value.length === 0;
      const atStart = isEmpty || (el.selectionStart === 0 && el.selectionEnd === 0);
      const atEnd = isEmpty || (el.selectionStart === value.length && el.selectionEnd === value.length);

      if (event.key === 'ArrowUp' && atStart && inputHistory.value.length > 0) {
        event.preventDefault();
        if (historyIndex.value === -1) {
          historyIndex.value = inputHistory.value.length;
        }
        historyIndex.value = Math.max(0, historyIndex.value - 1);
        messageInput.value = inputHistory.value[historyIndex.value] ?? '';
        requestAnimationFrame(() => {
          if (!el) return;
          el.selectionStart = el.selectionEnd = el.value.length;
        });
        return;
      }

      if (event.key === 'ArrowDown' && atEnd && inputHistory.value.length > 0 && historyIndex.value !== -1) {
        event.preventDefault();
        historyIndex.value = Math.min(inputHistory.value.length, historyIndex.value + 1);
        messageInput.value = historyIndex.value >= inputHistory.value.length ? '' : (inputHistory.value[historyIndex.value] ?? '');
        requestAnimationFrame(() => {
          if (!el) return;
          el.selectionStart = el.selectionEnd = el.value.length;
        });
        return;
      }
    }
  }

  if (event.key === 'Enter') {
    if (event.shiftKey) return;
    event.preventDefault();
    handleSendButtonClick();
  }
};

onMounted(() => {
  nextTick(() => {
    updateIsAtBottom();
  });
});

onUnmounted(() => {
  if (autosaveTimer.value) {
    clearTimeout(autosaveTimer.value);
    autosaveTimer.value = null;
  }
  stopGenerating();
  chatStore.clearStreamTarget();
});

const setMessageRef = (el, index) => {
  if (el) {
    messageRefs.value[index] = el;
    nextTick(() => {
      addCopyButtons(el);
      addMediaViewerButtons(el);
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
  position: relative;
}

.chat-header {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 14px;
  border-bottom: 1px solid var(--guaxi-border);
  background: var(--guaxi-panel-bg);
}

.header-select-btn {
  width: 30px;
  height: 30px;
  padding: 0;
  line-height: 0;
  border-radius: 999px;
  border: 1px solid var(--guaxi-border);
  background: var(--guaxi-panel-bg);
  color: var(--guaxi-text-secondary);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transform: translateY(-2px);
}

.header-select-btn:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}

.header-select-btn.active {
  color: #ffffff;
  background: #0ea5a4;
  border-color: #0ea5a4;
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
  padding: 16px 16px 8px;
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

.message-empty {
  padding: 18px 10px;
  color: var(--guaxi-text-secondary);
  font-size: 13px;
}

.select-toolbar {
  position: sticky;
  top: 0;
  z-index: 5;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 8px;
  margin-bottom: 10px;
  border: 1px solid var(--guaxi-border);
  border-radius: 10px;
  background: color-mix(in srgb, var(--guaxi-panel-bg) 92%, white 8%);
}

.select-toolbar-text {
  font-size: 12px;
  color: var(--guaxi-text-secondary);
}

.select-toolbar-btn {
  border: 1px solid var(--guaxi-border);
  border-radius: 8px;
  background: var(--guaxi-panel-bg);
  color: var(--guaxi-text-primary);
  font-size: 12px;
  padding: 4px 8px;
  cursor: pointer;
}

.select-toolbar-btn.primary {
  background: #0ea5a4;
  color: #fff;
  border-color: #0ea5a4;
}

.select-toolbar-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.message-left {
  flex-direction: row;
}

.message-right {
  flex-direction: row-reverse;
}

.msg-select-box {
  display: inline-flex;
  align-items: center;
  margin: 0 8px;
}

.message-bubble-selected {
  outline: 2px solid color-mix(in srgb, #0ea5a4 60%, var(--guaxi-border) 40%);
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
  position: relative;
  background: var(--guaxi-content-bg);
  padding: 0 12px 12px;
  min-height: 120px;
  font-size: 17px;
  font-family: 'Maple Mono NF CN', 'Noto Sans CJK SC', Consolas, 'Courier New', Courier, monospace;
}

.composer-shell {
  min-height: 96px;
  border: 1px solid var(--guaxi-border);
  border-radius: 20px;
  background: var(--guaxi-panel-bg);
  box-shadow: 0 10px 26px rgba(15, 23, 42, 0.1);
  padding: 10px 12px 8px;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.attachment-list {
  display: flex;
  gap: 6px;
  flex-wrap: wrap;
  max-height: 64px;
  overflow: auto;
}

.attachment-chip {
  max-width: 260px;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 3px 8px;
  border: 1px solid var(--guaxi-border);
  border-radius: 999px;
  background: var(--guaxi-panel-bg);
  font-size: 12px;
  color: var(--guaxi-text-secondary);
}

.attachment-name {
  max-width: 130px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.attachment-size {
  opacity: 0.85;
}

.attachment-remove {
  border: none;
  background: transparent;
  color: var(--guaxi-text-secondary);
  cursor: pointer;
  font-size: 14px;
  line-height: 1;
  padding: 0;
}

.composer-input {
  width: 100%;
  min-height: 72px;
  border: none;
  outline: none;
  box-shadow: none;
  background: transparent;
  color: var(--guaxi-text-primary);
  caret-color: var(--guaxi-text-primary);
  -webkit-text-fill-color: var(--guaxi-text-primary);
  font-size: 16px;
  line-height: 1.45;
  resize: none;
  padding: 0;
  margin: 0;
  font-family: inherit;
}

.composer-input:focus {
  caret-color: var(--guaxi-text-primary);
}

.composer-input::placeholder {
  color: var(--guaxi-text-secondary);
  opacity: 0.78;
}

.composer-toolbar {
  display: flex;
  align-items: center;
  gap: 8px;
}

.composer-icon-btn {
  width: 30px;
  height: 30px;
  padding: 0;
  line-height: 0;
  border-radius: 999px;
  border: 1px solid var(--guaxi-border);
  background: var(--guaxi-panel-bg);
  color: var(--guaxi-text-secondary);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
}

.composer-icon-btn svg {
  width: 17px;
  height: 17px;
  display: block;
  overflow: visible;
}

.composer-icon-btn:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}

.composer-icon-btn.active {
  color: #ffffff;
  background: #0ea5a4;
  border-color: #0ea5a4;
}

.translate-group {
  display: inline-flex;
  align-items: center;
  gap: 6px;
}

.translate-select {
  border: 1px solid var(--guaxi-border);
  border-radius: 999px;
  background: var(--guaxi-panel-bg);
  color: var(--guaxi-text-secondary);
  font-size: 12px;
  padding: 4px 10px;
  outline: none;
}

.composer-spacer {
  flex: 1 1 auto;
}

.send-btn {
  width: 36px;
  height: 36px;
  border-radius: 999px;
  border: none;
  background: #111827;
  color: #ffffff;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  box-shadow: 0 6px 14px rgba(17, 24, 39, 0.24);
}

.send-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.jump-to-bottom {
  position: absolute;
  left: 50%;
  top: -40px;
  transform: translateX(-50%);
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
  z-index: 6;
}

:root[data-guaxi-theme='dark'] .composer-shell,
.dark .composer-shell,
.dark-mode .composer-shell {
  background: var(--guaxi-panel-bg);
  box-shadow: 0 10px 26px rgba(0, 0, 0, 0.35);
}

:root[data-guaxi-theme='dark'] .input-area,
.dark .input-area,
.dark-mode .input-area {
  background: var(--guaxi-content-bg);
}

:root[data-guaxi-theme='dark'] .send-btn,
.dark .send-btn,
.dark-mode .send-btn {
  background: #e5e7eb;
  color: #111827;
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

.dark-mode .typing-dots i,
.dark .typing-dots i,
:root[data-guaxi-theme='dark'] .typing-dots i {
  background: rgba(255, 255, 255, 0.75);
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

.message-action-btn.copied {
  color: var(--guaxi-success, #2db55d);
  border-color: color-mix(in srgb, var(--guaxi-success, #2db55d), var(--guaxi-border) 40%);
}

.message-copy-hint {
  font-size: 12px;
  color: var(--guaxi-success, #2db55d);
  align-self: center;
}

.message-action-btn:disabled {
  cursor: not-allowed;
  opacity: 0.55;
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

/* Markdown链接样式（与 GuaxiEditor 保持一致） */
.message-content a,
.message-content a:visited {
  color: var(--guaxi-text-link, var(--primary-color, #0969da));
  text-decoration: underline;
  text-underline-offset: 2px;
}

.dark-mode .message-content a,
.dark-mode .message-content a:visited,
.dark .message-content a,
.dark .message-content a:visited,
:root[data-guaxi-theme='dark'] .message-content a,
:root[data-guaxi-theme='dark'] .message-content a:visited {
  color: var(--guaxi-text-link-dark, #4acadb);
}

.message-content a:hover {
  opacity: 0.92;
}

/* Mermaid图表样式 */
.mermaid {
  background-color: #ffffff;
  color: #1f2328;
  border-radius: 8px;
  padding: 16px;
  margin: 1em 0;
  text-align: center;
  border: 1px solid var(--guaxi-border);
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace;
}

:root[data-guaxi-theme='dark'] .mermaid svg {
  background-color: #ffffff;
}

/* 代码块样式 */
.hljs {
  position: relative;
  border-radius: 6px;
  padding: 1.5em 1em !important;
  font-size: 17px;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace;
  background: var(--guaxi-content-bg) !important;
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

