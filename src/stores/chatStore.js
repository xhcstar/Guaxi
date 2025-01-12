import { defineStore } from "pinia";

export const useChatStore = defineStore("chatStore", {
  state: () => ({
    chatSessions: [
      {
        id: "1",
        name: "猫蛋",
        avatar: "https://i.pravatar.cc/100?img=26",
        lastMessage: "你好！",
        lastMessageTime: Date.now(),
        isMine: false,
        messages: [
          { id: "m1", sender: "bot", avatar: "https://i.pravatar.cc/100?img=26", content: "你好，有什么可以帮忙的吗？" },
          { id: "m1", sender: "me", avatar: "https://i.pravatar.cc/100?img=4", content: "我叫瓜西" },
          { id: "m1", sender: "bot", avatar: "https://i.pravatar.cc/100?img=26", content: "你为什叫瓜西？" },
          { id: "m1", sender: "me", avatar: "https://i.pravatar.cc/100?img=4", content: "你管！" },
        ],
      },
      {
        id: "2",
        name: "狗蛋",
        avatar: "https://i.pravatar.cc/100?img=2",
        lastMessage: "最近怎么样？",
        lastMessageTime: Date.now() - 60000,
        messages: [
          { id: "m2", sender: "bot", avatar: "https://i.pravatar.cc/100?img=2", content: "你好，有什么可以帮忙的吗？" },
          { id: "m2", sender: "me", avatar: "https://i.pravatar.cc/100?img=4", content: "我叫瓜西" },
          { id: "m2", sender: "bot", avatar: "https://i.pravatar.cc/100?img=2", content: "你为什叫瓜西？" },
          { id: "m2", sender: "me", avatar: "https://i.pravatar.cc/100?img=4", content: "你管！" },
        ],
      },
    ],
    activeChatId: "1",
  }),
  actions: {
    // 更新激活的会话 ID
    setActiveChat(chatId) {
      this.activeChatId = chatId;
    },
    updateChatSession(id, lastMessage, lastMessageTime) {
      const session = this.chatSessions.find((chat) => chat.id === id);
      if (session) {
        session.lastMessage = lastMessage;
        session.lastMessageTime = lastMessageTime;
      }
    },
    addChatSession(newSession) {
      this.chatSessions.push(newSession);
    },
  },
});
