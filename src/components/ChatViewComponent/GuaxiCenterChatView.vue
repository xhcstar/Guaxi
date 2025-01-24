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
          <div class="message-content" v-html="renderMarkdown(message.content)"></div>
        </div>
      </div>

    </div>

    <div class="input-area">
      <div class="chat-input-height-handle" @mousedown="startResize"></div>
      <n-input type="textarea" placeholder="请输入您的问题..." v-model:value="messageInput" @keydown="handleKeydown"
        style="flex-grow: 1; height: 100%; padding: 2px; border-radius: 5px; box-sizing: border-box;" />
      <n-button @click="sendMessage" color="#dbe7e4"
        style="position: absolute; right: 20px; bottom: 20px; border-radius: 50%; padding: 0 20px; height: 36px; min-width: 36px; font-size: 12px;">
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
import { ref, nextTick, computed, onMounted } from 'vue'
import { marked } from 'marked';
import katex from 'katex'; // 导入 KaTeX 库
import 'katex/dist/katex.min.css'; // 导入 KaTeX 样式
import Prism from 'prismjs';
import 'prismjs/components/prism-python.min.js';  // 引入 Python 支持
import 'prismjs/themes/prism.css';  // 引入 Prism 默认样式
import { useChatStore } from "../../stores/chatStore";
import { invoke } from '@tauri-apps/api/core';


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

const askDeepSeek = async (question) => {
  try {
    const request = {
      "modelid": 1,
      "question": question,
      "topicid": 1,
    };
    console.log('Request:', request);
    const response = await invoke('get_chat_answer', { request });
    console.log('Response:', response);
    return response.answer;
  } catch (error) {
    console.error('Error calling API:', error);
  }
};

// 发送消息
const sendMessage = async () => {
  if (!messageInput.value.trim() || !activeChat.value) return;

  let question = messageInput.value.trim();
  activeChatMessages.value.topic.push({
    msgid: activeChatMessages.value.topic.length + 1,
    sender: "me",
    content: question,
  });

  messageInput.value = ""; // 清空输入框
  scrollToBottom(); // 滚动到底部

  // 更新当前会话的消息记录
  try {
    let answer = await askDeepSeek(question); // 等待 Promise 解析
    console.log('answer:', answer);
    chatStore.updateChatSession(answer);
  } catch (error) {
    console.error('Error during askDeepSeek:', error);
  }
  scrollToBottom(); // 滚动到底部
};

const handleKeydown = (event) => {
  if (event.key === 'Enter') {
    if (event.shiftKey) {
      // Shift + Enter 换行，什么都不做
      return;
    }
    // 阻止默认行为并发送消息
    event.preventDefault();
    sendMessage();
  }
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

function renderMath(html) {
  // 匹配 LaTeX 表达式，包括以下情况：
  // 1. $$...$$ 块级公式
  // 2. $...$ 内联公式
  // 3. [ ... ] 标记的公式
  // 4. (\ ... \) 标记的公式
  const latexRegex = /\$\$(.+?)\$\$|\$(.+?)\$|\[(.+?)\]|\(\s*\\(.+?)\s*\)/g;

  html = html.replace(latexRegex, (match, blockMath, inlineMath, bracketMath, parenMath) => {
    let mathContent = blockMath || inlineMath || bracketMath || parenMath;
    try {
      // 根据类型选择块级或内联渲染
      if (blockMath) {
        return `<div class="katex-display">${katex.renderToString(mathContent)}</div>`;
      } else {
        return `<span class="katex-inline">${katex.renderToString(mathContent)}</span>`;
      }
    } catch (e) {
      console.error("KaTeX 渲染错误：", e);
      return match; // 如果渲染失败，返回原始文本
    }
  });

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
  max-height: calc(100% - 150px - 80px);
  /* 限制消息区域的最大高度 */
  overflow-y: auto;
  /* 超出时显示滚动条 */
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
  background-color: #5acaa5;
  color: white;
  border-top-right-radius: 0;
}

.input-area {
  margin-top: 1px;
  padding-top: 1px;
  border-top: 1px solid #ddd;
  position: relative;
  padding-bottom: 2px;
  height: 150px;
  /* 固定输入区域的高度 */
  min-height: 80px;
  /* 设置最小高度 */
}

.chat-input-height-handle {
  position: relative;
  width: 100%;
  height: 2px;
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
