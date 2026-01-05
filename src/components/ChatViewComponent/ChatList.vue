<template>
  <div class="chat-list-container">
    <div class="chat-list-header">
      <div style="flex-grow: 2; align-items: center; vertical-align: middle;">
        <n-input round size="small" placeholder="搜索" />
      </div>
      <div>
        <n-icon size="20" style="flex-grow: 1; align-items: center; vertical-align: middle;" @click="handleAddChat">
          <svg t="1736598327807" class="icon" viewBox="0 0 1024 1024" version="1.1" xmlns="http://www.w3.org/2000/svg"
            p-id="21966" width="20" height="20">
            <path
              d="M146.311835 1024a146.275266 146.275266 0 0 1-146.275266-146.275266v-731.37633a146.275266 146.275266 0 0 1 146.275266-146.275266h731.37633a146.275266 146.275266 0 0 1 146.275266 146.275266v731.37633a146.275266 146.275266 0 0 1-146.275266 146.275266z m-73.137633-877.651596v731.37633a73.137633 73.137633 0 0 0 73.137633 73.137633h731.37633a73.137633 73.137633 0 0 0 73.137633-73.137633v-731.37633a73.137633 73.137633 0 0 0-73.137633-73.137633H146.311835a73.137633 73.137633 0 0 0-73.137633 73.137633z m399.624027 600.752517V551.165202H276.935647a39.128634 39.128634 0 0 1-39.128633-39.128633 39.274909 39.274909 0 0 1 39.128633-39.274909h195.862582V276.899079a39.274909 39.274909 0 0 1 39.274909-39.128634 39.128634 39.128634 0 0 1 39.128633 39.128634v195.862581h196.008857a39.274909 39.274909 0 0 1 39.128633 39.274909 39.128634 39.128634 0 0 1-39.128633 39.128633h-196.008857v195.935719a39.128634 39.128634 0 0 1-39.128633 39.128634 39.274909 39.274909 0 0 1-39.348047-39.055496z"
              fill="#6A6A6A" p-id="21967"></path>
          </svg>
        </n-icon>
      </div>
    </div>

    <div class="chat-list">
      <div v-for="curtopic in (activeChat?.MessagesList || [])" :key="curtopic.topicid" class="chat-item"
        :class="{ active: curtopic.topicid === chatStore.selectedTopic }" @click="selectChat(curtopic.topicid)"
        @contextmenu="showContextMenu($event, curtopic)">
        <div class="chat-avatar">
          <n-avatar :src="activeChat?.ModelAvatar" size="medium" round />
        </div>
        <div class="chat-info">
          <div class="chat-title">
            {{ curtopic?.title || activeChat?.name }}
            <!-- <span class="chat-timestamp">{{ formatTimestamp(curmodel.MessagesList.at(0)) }}</span> -->
          </div>
          <div class="chat-last-message">
            {{ curtopic.topic?.[0]?.content || '' }}
          </div>
        </div>
      </div>
    </div>

    <!-- 右键菜单 -->
    <div v-if="showMenu" class="context-menu" :style="{ top: menuTop, left: menuLeft }">
      <div class="menu-item" @click="renameChatItem">重命名</div>
      <div class="menu-item" @click="saveChatItem">保存对话</div>
      <div class="menu-item" @click="deleteChatItem">删除会话</div>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, onUnmounted } from "vue"
import { useChatStore } from "../../stores/chatStore";
import { useMessage } from "naive-ui";
import { invoke } from '@tauri-apps/api/core';

const chatStore = useChatStore();
const message = useMessage();
const activeChat = computed(() => chatStore.SelectModelchatSessions.find((chatmodel) => chatmodel.id === chatStore.selectedModel));

const showMenu = ref(false);
const menuTop = ref(0);
const menuLeft = ref(0);
const selectedChat = ref(null);

const selectChat = (id) => {
  chatStore.setModelTopic(id);
};

// 新建聊天逻辑（可自定义实现）
const handleAddChat = () => {
  chatStore.addChatSession()
};

// 显示右键菜单
const showContextMenu = (event, chat) => {
  event.preventDefault();
  selectedChat.value = chat;
  showMenu.value = true;
  menuTop.value = event.clientY + "px";
  menuLeft.value = event.clientX + "px";
};

const getTopicSessionId = (topic) => {
  const items = Array.isArray(topic) ? topic : [];
  for (let i = items.length - 1; i >= 0; i--) {
    const sid = items[i]?.chatSessionId;
    if (typeof sid === 'string' && sid.trim()) return sid;
  }
  return '';
};

const persistSelectedChat = async () => {
  if (!selectedChat.value || !activeChat.value) return;

  const content = Array.isArray(selectedChat.value?.topic) ? selectedChat.value.topic : [];
  const sessionId = getTopicSessionId(content);
  const topicTitle = (selectedChat.value?.title ?? '').toString().trim() || null;

  await invoke('save_chat_session', {
    request: {
      model_id: activeChat.value.id,
      model_name: activeChat.value?.name || '',
      topic_id: selectedChat.value.topicid,
      session_id: sessionId || null,
      topic_title: topicTitle,
      content,
    },
  });
};

const renameChatItem = async () => {
  try {
    if (!selectedChat.value || !activeChat.value) return;

    const current = (selectedChat.value?.title ?? '').toString().trim();
    const nextTitle = window.prompt('重命名会话', current || '');
    if (nextTitle === null) return;

    const t = nextTitle.toString().trim();
    if (t) {
      selectedChat.value.title = t;
      selectedChat.value.__titleLocked = true;
    } else {
      selectedChat.value.title = '';
      selectedChat.value.__titleLocked = false;
    }

    await persistSelectedChat();
    message.success('已更新会话名称');
  } catch (e) {
    console.error('重命名失败:', e);
    message.error('重命名失败');
  } finally {
    showMenu.value = false;
  }
};

const saveChatItem = async () => {
  try {
    if (!selectedChat.value || !activeChat.value) return;

    const content = Array.isArray(selectedChat.value?.topic) ? selectedChat.value.topic : [];
    const sessionId = getTopicSessionId(content);
    const topicTitle = (selectedChat.value?.title ?? '').toString().trim() || null;

    const filePath = await invoke('export_chat_session', {
      request: {
        model_id: activeChat.value.id,
        model_name: activeChat.value?.name || '',
        topic_id: selectedChat.value.topicid,
        session_id: sessionId,
        topic_title: topicTitle,
        content,
      }
    })

    message.success(`已保存到: ${filePath}`)
  } catch (e) {
    console.error('保存对话失败:', e)
    message.error('保存对话失败')
  } finally {
    showMenu.value = false;
  }
};

// 删除聊天条目
const deleteChatItem = () => {
  if (!selectedChat.value || !activeChat.value) {
    showMenu.value = false;
    return;
  }

  const deleteTopicId = selectedChat.value.topicid;
  activeChat.value.MessagesList = (activeChat.value.MessagesList || []).filter(
    (topic) => topic?.topicid !== deleteTopicId
  );

  if (chatStore.selectedTopic === deleteTopicId) {
    const fallbackTopicId = activeChat.value.MessagesList?.[0]?.topicid;
    if (typeof fallbackTopicId === 'number') {
      chatStore.setModelTopic(fallbackTopicId);
    } else {
      chatStore.addChatSession();
      chatStore.setModelTopic(activeChat.value.MessagesList.length - 1);
    }
  }

  showMenu.value = false;
};

// 监听全局点击事件，点击其他地方关闭菜单
const closeContextMenu = (event) => {
  if (!event.target.closest(".context-menu")) {
    showMenu.value = false;
  }
};

// 组件挂载时添加监听
onMounted(() => {
  document.addEventListener("click", closeContextMenu);
});

// 组件卸载时移除监听
onUnmounted(() => {
  document.removeEventListener("click", closeContextMenu);
});

</script>

<style scoped>
.chat-list-container {
  display: flex;
  flex-direction: column;
  width: 100%;
  height: 100%;
  border-right: 1px solid rgba(15, 23, 42, 0.08);
  background: linear-gradient(180deg, #ffffff 0%, #fbfdff 100%);
  overflow-y: auto;
}

.chat-list-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 12px 14px;
  border-bottom: 1px solid rgba(15, 23, 42, 0.08);
  background: rgba(255, 255, 255, 0.9);
  backdrop-filter: blur(10px);
  gap: 8px;
}

.chat-list {
  flex-grow: 1;
  overflow-y: auto;
}

.chat-item {
  display: flex;
  align-items: center;
  padding: 10px 12px;
  cursor: pointer;
  transition: background-color 0.18s ease, border-color 0.18s ease;
  border-radius: 12px;
  margin: 6px 10px;
  border: 1px solid transparent;
}

.chat-item.active {
  background: rgba(45, 212, 191, 0.12);
  border-color: rgba(45, 212, 191, 0.25);
}

.chat-item:hover {
  background: rgba(15, 23, 42, 0.04);
}

.chat-avatar {
  flex: 0 0 auto;
}

.chat-info {
  flex: 1 1 auto;
  min-width: 0;
  margin-left: 10px;
  display: flex;
  flex-direction: column;
}

.chat-title {
  font-size: 14px;
  font-weight: bold;
  color: #333;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.chat-timestamp {
  font-size: 12px;
  color: #999;
  margin-left: 10px;
}

.chat-last-message {
  font-size: 12px;
  color: #666;
  margin-top: 2px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

/* 右键菜单 */
.context-menu {
  position: absolute;
  background: rgba(255, 255, 255, 0.98);
  border: 1px solid rgba(15, 23, 42, 0.12);
  box-shadow: 0 10px 30px rgba(15, 23, 42, 0.12);
  border-radius: 10px;
  padding: 6px;
  z-index: 1000;
  min-width: 140px;
}

.menu-item {
  padding: 9px 12px;
  cursor: pointer;
  font-size: 13px;
  border-radius: 8px;
}

.menu-item:hover {
  background: rgba(15, 23, 42, 0.06);
}
</style>
