import { defineStore } from "pinia";

export const useChatStore = defineStore("chatStore", {
  state: () => ({
    SelectModelchatSessions: [
      {
        id: 0,
        name: "ChatGPT",
        ModelAvatar: "https://i.pravatar.cc/100?img=26",
        MeAvatar: "https://i.pravatar.cc/100?img=4",
        MessagesList: [
          //Messages1
          {
            topicid: 0,
            topic: [
              {
                msgid: 1,
                sender: "ChatGPT",
                content: "你好，有什么可以帮忙的吗？",
              },
              {
                msgid: 2,
                sender: "me",
                content: "我叫瓜西",
              },
              {
                msgid: 3,
                sender: "ChatGPT",
                content: "你为什叫瓜西？",
              },
              {
                msgid: 4,
                sender: "me",
                content: "你管！",
              },
            ],
          },
          //Messages2
          {
            topicid: 1,
            topic: [
              {
                msgid: 1,
                sender: "ChatGPT",
                content: "你好, 我是ChatGPT, 有什么可以帮忙的吗？",
              },
              {
                msgid: 2,
                sender: "me",
                content: "我叫Siri",
              },
              {
                msgid: 3,
                sender: "ChatGPT",
                content: "你为什叫Siri?",
              },
              {
                msgid: 4,
                sender: "me",
                content: "你管！",
              },
            ],
          },
        ],
      },

      {
        id: 1,
        name: "DeepSeek",
        ModelAvatar: "https://i.pravatar.cc/100?img=2",
        MeAvatar: "https://i.pravatar.cc/100?img=4",
        MessagesList: [
          //Messages1
          {
            topicid: 0,
            topic: [
              {
                msgid: 1,
                sender: "DeepSeek",
                content: "你好，有什么可以帮忙的吗？",
              },
              {
                msgid: 2,
                sender: "me",
                content: "我叫瓜西",
              },
              {
                msgid: 3,
                sender: "DeepSeek",
                content: "你为什叫瓜西？",
              },
              {
                msgid: 4,
                sender: "me",
                content: "你管！",
              },
            ],
          },
          //Messages2
          {
            topicid: 1,
            topic: [
              {
                msgid: 1,
                sender: "DeepSeek",
                content: "你好, 我是DeepSeek, 有什么可以帮忙的吗？",
              },
              {
                msgid: 2,
                sender: "me",
                content: "我叫瓜西",
              },
              {
                msgid: 3,
                sender: "DeepSeek",
                content: "你为什叫瓜西？",
              },
              {
                msgid: 4,
                sender: "me",
                content: "你管！",
              },
            ],
          },
        ],
      },

      {
        id: 2,
        name: "Qwen",
        ModelAvatar: "https://i.pravatar.cc/100?img=5",
        MeAvatar: "https://i.pravatar.cc/100?img=4",
        MessagesList: [
          //Messages1
          {
            topicid: 0,
            topic: [
              {
                msgid: 1,
                sender: "Qwen",
                content: "你好, 我是Qwen, 有什么可以帮忙的吗？",
              },
              {
                msgid: 2,
                sender: "me",
                content: "我叫瓜西",
              },
              {
                msgid: 3,
                sender: "Qwen",
                content: "你为什叫瓜西？",
              },
              {
                msgid: 4,
                sender: "me",
                content: "你管！",
              },
            ],
          },
        ],
      },

      {
        id: 3,
        name: "Kimi",
        ModelAvatar: "https://i.pravatar.cc/100?img=6",
        MeAvatar: "https://i.pravatar.cc/100?img=4",
        MessagesList: [
          //Messages1
          {
            topicid: 0,
            topic: [
              {
                msgid: 1,
                sender: "Kimi",
                content: "你好, 我是Kimi, 有什么可以帮忙的吗？",
              },
              {
                msgid: 2,
                sender: "me",
                content: "我叫瓜西",
              },
              {
                msgid: 3,
                sender: "Kimi",
                content: "你为什叫瓜西？",
              },
              {
                msgid: 4,
                sender: "me",
                content: "你管！",
              },
            ],
          },
        ],
      },

      {
        id: 4,
        name: "豆包",
        ModelAvatar: "https://i.pravatar.cc/100?img=7",
        MeAvatar: "https://i.pravatar.cc/100?img=4",
        MessagesList: [
          //Messages1
          {
            topicid: 0,
            topic: [
              {
                msgid: 1,
                sender: "豆包",
                content: "你好, 我是豆包, 有什么可以帮忙的吗？",
              },
              {
                msgid: 2,
                sender: "me",
                content: "我叫瓜西",
              },
              {
                msgid: 3,
                sender: "豆包",
                content: "你为什叫瓜西？",
              },
              {
                msgid: 4,
                sender: "me",
                content: "你管！",
              },
            ],
          },
        ],
      },

      {
        id: 10,
        name: "Local-Qwen",
        ModelAvatar: "https://i.pravatar.cc/100?img=8",
        MeAvatar: "https://i.pravatar.cc/100?img=4",
        MessagesList: [
          //Messages1
          {
            topicid: 0,
            topic: [
              {
                msgid: 1,
                sender: "Local-Qwen",
                content: "你好，有什么可以帮忙的吗？",
              },
              {
                msgid: 2,
                sender: "me",
                content: "我叫瓜西",
              },
              {
                msgid: 3,
                sender: "Local-Qwen",
                content: "你为什叫瓜西？",
              },
              {
                msgid: 4,
                sender: "me",
                content: "你管！",
              },
            ],
          },
        ],
      },

      {
        id: 11,
        name: "Local-LLAMA",
        ModelAvatar: "https://i.pravatar.cc/100?img=9",
        MeAvatar: "https://i.pravatar.cc/100?img=4",
        MessagesList: [
          //Messages1
          {
            topicid: 0,
            topic: [
              {
                msgid: 1,
                sender: "Local-LLAMA",
                content: "你好，有什么可以帮忙的吗？",
              },
              {
                msgid: 2,
                sender: "me",
                content: "我叫瓜西",
              },
              {
                msgid: 3,
                sender: "Local-LLAMA",
                content: "你为什叫瓜西？",
              },
              {
                msgid: 4,
                sender: "me",
                content: "你管！",
              },
            ],
          },
        ],
      },
    ],

    activeChatId: 0,
    selectedModel: 0,
    selectedTopic: 0,

    LLMlist: {
      remoteModel: [
        { name: "ChatGPT", selectedModel: "0" },
        { name: "DeekSeek", selectedModel: "1" },
        { name: "Qwen", selectedModel: "2" },
        { name: "Kimi", selectedModel: "3" },
        { name: "豆包", selectedModel: "4" },
      ],

      localModel: [
        { name: "Local-Qwen", selectedModel: "10" },
        { name: "Local-LLAMA", selectedModel: "11" },
      ],
    },
  }),
  actions: {
    // 更新激活的会话 ID
    setActiveChat(chatId) {
      this.activeChatId = chatId;
    },

    setChatModel(modelId) {
      this.selectedModel = modelId;
    },

    setModelTopic(topicId) {
      this.selectedTopic = topicId;
    },
    // updateChatSession(id) {
    //   const chatmodel = this.SelectModelchatSessions.find((model) => model.id === id);
    //   const seion = chatmodel.MessagesList[session.MessagesList.length - 1];
    //   if (topic) {
    //     topic.lastMessage = lastMessage;
    //   }
    // },
    
    addChatSession(newSession) {
      this.SelectModelchatSessions.push(newSession);
    },
  },
});
