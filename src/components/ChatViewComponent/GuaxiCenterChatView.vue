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
          <!-- <div class="message-content">{{ message.content }}</div> -->
          <div class="message-content" v-html="renderMarkdown(message.content)"></div>
        </div>
      </div>

    </div>

    <div class="input-area">
      <div class="chat-input-height-handle" @mousedown="startResize"></div>
      <n-input type="textarea" placeholder="请输入您的问题..." v-model:value="messageInput" @keypress.enter="sendMessage"
        style="flex-grow: 1; height: 100%; padding: 2px; border-radius: 5px; box-sizing: border-box;" />
      <n-button @click="sendMessage" type="primary"
        style="position: absolute; right: 20px; bottom: 20px; border-radius: 50%; padding: 0 20px; height: 36px; min-width: 36px; font-size: 12px;">
        发送
      </n-button>
    </div>
  </div>
</template>

<script setup>
import { ref, nextTick, computed, onMounted } from 'vue'
import { marked } from 'marked';
import katex from 'katex'; // 导入 KaTeX 库
import 'katex/dist/katex.min.css'; // 导入 KaTeX 样式
import Prism from 'prismjs';
import 'prismjs/components/prism-python.min.js';  // 引入 Python 支持
import 'prismjs/themes/prism.css';  // 引入 Prism 默认样式
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

function renderMarkdown(content) {
  nextTick(() => {
    Prism.highlightAll();
  });
  // 先将 Markdown 转换为 HTML
  let html = marked(content);

  // 渲染数学公式
  html = renderMath(html);

  // 渲染代码块
  html = renderCodeBlocks(html);

  return html;
}

// 渲染 LaTeX 数学公式
function renderMath(html) {
  // 渲染内联数学公式（$...$）和块级数学公式（$$...$$）
  const mathElements = html.match(/(\$\$.*?\$\$|\$.*?\$)/g);
  if (mathElements) {
    mathElements.forEach((element) => {
      html = html.replace(element, (match) => {
        if (match.startsWith('$$')) {
          return `<div class="katex-display">${katex.renderToString(match.slice(2, -2))}</div>`;
        } else {
          return `<span class="katex-inline">${katex.renderToString(match.slice(1, -1))}</span>`;
        }
      });
    });
  }
  return html;
}

const renderCodeBlocks = (content) => {
  const codeBlocks = content.match(/```[\s\S]+?```/g) || [];

  codeBlocks.forEach((block) => {
    const language = block.match(/```(\w+)\n/);  // 获取语言类型
    if (language && language[1]) {
      const lang = language[1].toLowerCase();
      if (Prism.languages[lang]) {
        block = block.replace(/```(\w+)\n([\s\S]+?)```/g, (match, p1, p2) => {
          const highlightedCode = Prism.highlight(p2, Prism.languages[lang], lang);
          return `<pre class="code-block"><code class="language-${lang}">${highlightedCode}</code></pre>`;
        });
      }
    }
  });

  return content.replace(/```[\s\S]+?```/g, (block) => {
    return `<pre class="code-block">${block}</pre>`;
  });
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

.katex-display {
  display: block;
  margin: 10px 0;
}

.katex-inline {
  font-size: 1.2em;
  margin-left: 5px;
  margin-right: 5px;
}

/* 代码块滚动区域样式 */
.code-block {
  background-color: #2d2d2d;
  color: #f8f8f2;
  padding: 16px;
  border-radius: 4px;
  overflow-x: auto;
  max-height: 300px;
  white-space: pre-wrap;
  /* 保持代码格式 */
  word-wrap: break-word;
  font-family: 'Courier New', monospace;
  margin-top: 10px;
}
</style>
