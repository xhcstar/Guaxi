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
      <div v-for="curtopic in activeChat.MessagesList" :key="curtopic.topicid" class="chat-item"
        :class="{ active: activeChat.id === chatStore.selectedModel }" @click="selectChat(curtopic.topicid)"
        @contextmenu="showContextMenu($event, curtopic)">
        <n-avatar :src="activeChat.ModelAvatar" size="medium" round />
        <div class="chat-info">
          <div class="chat-title">
            {{ activeChat.name }}
            <!-- <span class="chat-timestamp">{{ formatTimestamp(curmodel.MessagesList.at(0)) }}</span> -->
          </div>
          <div class="chat-last-message">
            {{ curtopic.topic.at(0).content }}
          </div>
        </div>
      </div>
    </div>

    <!-- 右键菜单 -->
    <div v-if="showMenu" class="context-menu" :style="{ top: menuTop, left: menuLeft }">
      <div class="menu-item" @click="deleteChatItem">删除会话</div>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, onUnmounted } from "vue"
import { useChatStore } from "../../stores/chatStore";
import { NAvatar, NButton } from "naive-ui";

const chatStore = useChatStore();
const activeChat = computed(() => chatStore.SelectModelchatSessions.find((chatmodel) => chatmodel.id === chatStore.selectedModel));

// 当前会话的消息记录
const activeChatMessages = computed(() => activeChat.value?.MessagesList.find((topic) => topic.topicid === chatStore.selectedTopic) || []);

const showMenu = ref(false);
const menuTop = ref(0);
const menuLeft = ref(0);
const selectedChat = ref(null);

const selectChat = (id) => {
  chatStore.setModelTopic(id);
};

// 格式化时间戳
const formatTimestamp = (timestamp) => {
  if (!timestamp) return "";
  const date = new Date(timestamp);
  return `${date.getHours()}:${String(date.getMinutes()).padStart(2, "0")}`;
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

// 删除聊天条目
const deleteChatItem = () => {
  if (activeChatMessages.value) {
    activeChat.value.MessagesList = activeChat.value.MessagesList.filter(
      (chat) => chat !== activeChatMessages.value
    );
  }
  showMenu.value = false; // 隐藏菜单
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
  border: 1px solid #ddd;
  background: #fff;
  overflow-y: auto;
}

.chat-list-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 10px 15px;
  border-bottom: 1px solid #ddd;
  background-color: #f8f9fa;
  gap: 8px;
}

.chat-list {
  flex-grow: 1;
  overflow-y: auto;
}

.chat-item {
  display: flex;
  align-items: center;
  padding: 10px 15px;
  cursor: pointer;
  border-bottom: 1px solid #f1f1f1;
  transition: background-color 0.3s ease;
}

.chat-item.active {
  background-color: #e6f7ff;
}

.chat-item:hover {
  background-color: #f5f5f5;
}

.chat-info {
  flex-grow: 1;
  margin-left: 10px;
  display: flex;
  flex-direction: column;
}

.chat-title {
  font-size: 14px;
  font-weight: bold;
  color: #333;
  display: flex;
  justify-content: space-between;
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
  background: white;
  border: 1px solid #ccc;
  box-shadow: 0 2px 10px rgba(0, 0, 0, 0.2);
  border-radius: 4px;
  padding: 5px 0;
  z-index: 1000;
}

.menu-item {
  padding: 10px 20px;
  cursor: pointer;
  font-size: 14px;
}

.menu-item:hover {
  background: #f2f2f2;
}
</style>
