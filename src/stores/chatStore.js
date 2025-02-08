import { defineStore } from "pinia";

export const useChatStore = defineStore("chatStore", {
  state: () => ({
    SelectModelchatSessions: [
      {
        id: 0,
        name: "ChatGPT",
        ModelAvatar: "./src/assets/chatgpt.png",
        MeAvatar: "./src/assets/me.png",
        MessagesList: [
          {
            topicid: 0,
            topic: [
              {
                msgid: 0,
                sender: "ChatGPT",
                chatSessionId: "",
                content: "您好, 我是ChatGPT,有什么可以帮忙的吗?",
              },
            ],
          },
        ],
      },

      {
        id: 1,
        name: "DeepSeek",
        ModelAvatar: "./src/assets/deepseek.png",
        MeAvatar: "./src/assets/me.png",
        MessagesList: [
          {
            topicid: 0,
            topic: [
              {
                msgid: 0,
                sender: "DeepSeek",
                chatSessionId: "",
                content: "您好, 有什么可以帮忙的吗?",
              },
            ],
          },
        ],
      },

      {
        id: 2,
        name: "Qwen",
        ModelAvatar: "./src/assets/qwen.png",
        MeAvatar: "./src/assets/me.png",
        MessagesList: [
          {
            topicid: 0,
            topic: [
              {
                msgid: 0,
                sender: "Qwen",
                chatSessionId: "",
                content: "您好, 我是通义千问, 有什么可以帮忙的吗?",
              },
            ],
          },
        ],
      },

      {
        id: 3,
        name: "Kimi",
        ModelAvatar: "./src/assets/kimi.png",
        MeAvatar: "./src/assets/me.png",
        MessagesList: [
          {
            topicid: 0,
            topic: [
              {
                msgid: 0,
                sender: "Kimi",
                chatSessionId: "",
                content: "您好, 我是Kimi, 有什么可以帮忙的吗?",
              },
            ],
          },
        ],
      },

      {
        id: 4,
        name: "豆包",
        ModelAvatar: "./src/assets/doubao.png",
        MeAvatar: "./src/assets/me.png",
        MessagesList: [
          {
            topicid: 0,
            topic: [
              {
                msgid: 0,
                sender: "豆包",
                chatSessionId: "",
                content: "您好, 我是豆包, 有什么可以帮忙的吗?",
              },
            ],
          },
        ],
      },

      {
        id: 5,
        name: "Local-Qwen",
        ModelAvatar: "./src/assets/qwen.png",
        MeAvatar: "./src/assets/me.png",
        MessagesList: [
          {
            topicid: 0,
            topic: [
              {
                msgid: 0,
                sender: "Local-Qwen",
                chatSessionId: "",
                content: "您好, 我是通义千问, 有什么可以帮忙的吗?",
              },
            ],
          },
        ],
      },

      {
        id: 6,
        name: "Local-DeepSeek",
        ModelAvatar: "./src/assets/ollama.png",
        MeAvatar: "./src/assets/me.png",
        MessagesList: [
          {
            topicid: 0,
            topic: [
              {
                msgid: 0,
                sender: "Local-DeepSeek",
                chatSessionId: "",
                content: "您好, 我是DeepSeek, 有什么可以帮忙的吗?",
              },
            ],
          },
        ],
      },
    ],

    activeChatId: 0,
    selectedModel: 0,
    selectedTopic: 0,
    sendChatSessionId: "",

    LLMlist: {
      remoteModel: [
        { name: "ChatGPT", selectedModel: "0" },
        { name: "DeekSeek", selectedModel: "1" },
        { name: "Qwen", selectedModel: "2" },
        { name: "Kimi", selectedModel: "3" },
        { name: "豆包", selectedModel: "4" },
      ],

      localModel: [
        { name: "Local-Qwen", selectedModel: "5" },
        { name: "Local-DeepSeek", selectedModel: "6" },
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

    updateChatSession(answer) {
      console.log("answer", answer);
      console.log("this.selectedModel", this.selectedModel);
      console.log("this.selectedTopic", this.selectedTopic);
      this.SelectModelchatSessions.find((session) => {
        console.log("session", session);
        if (session.id === this.selectedModel) {
          session.MessagesList.find((topicitem) => {
            if (topicitem.topicid === this.selectedTopic) {
              console.log("topicitem", topicitem);
              topicitem.topic.push({
                msgid: topicitem.topic.length + 1,
                sender: session.name,
                chatSessionId: answer.sessionid,
                content: answer.answer,
              });
              this.sendChatSessionId = answer.sessionid;
            }
          });
        }
      });
    },

    collectChatSessionStream(answer) {
      console.log("answer", answer);
      console.log("this.selectedModel", this.selectedModel);
      console.log("this.selectedTopic", this.selectedTopic);
      this.SelectModelchatSessions.find((session) => {
        console.log("session", session);
        if (session.id === this.selectedModel) {
          session.MessagesList.find((topicitem) => {
            if (topicitem.topicid === this.selectedTopic) {
              console.log("topicitem", topicitem);
              const latestTopic = topicitem.topic[topicitem.topic.length - 1]; // 获取最后一个消息条目
              // 检查是否已经有内容
              if (latestTopic && latestTopic.sender !== "me") {
                latestTopic.content += `${answer}`;
              } else {
                // 如果没有消息条目，就创建一个新的
                topicitem.topic.push({
                  msgid: topicitem.topic.length,
                  sender: session.name,
                  content: answer,
                });
              }
              answer = "";
            }
          });
        }
      });
    },

    addChatSession() {
      this.SelectModelchatSessions.find((session) => {
        console.log("Add new chat", session);
        if (session.id === this.selectedModel) {
          session.MessagesList.push({
            topicid: session.MessagesList.length,
            topic: [
              {
                msgid: 0,
                sender: session.name,
                chatSessionId: "",
                content: "您好，我是`${session.name}`,有什么可以帮忙的吗？",
              },
            ],
          });
        }
      });
    },
  },
});
