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
                content: "# 示例标题\n\n这是一个包含 **Markdown**、数学公式和代码块的大段文本。以下是几种类型的内容：\n\n## 1. Markdown 格式\nMarkdown 格式可以使用很多常见的语法。比如：\n- **加粗**，*斜体*，~~删除线~~\n- 列表：\n  - 第一项\n  - 第二项\n  - 第三项\n- 链接：[Google](https://www.google.com)\n\n### 代码块（不转义引号）\n以下是一个示例代码块，它展示了如何显示引号并保持格式不变：\n\n```javascript\nconst message = \"Hello, world!\";\nconsole.log(message);  // 输出: Hello, world!\n```\n\n## 2. 数学公式\nMarkdown 也支持数学公式。你可以使用 `$$` 来写块级公式，或者使用 `$` 来写行内公式。\n\n### 行内公式\n行内公式示例：$E = mc^2$\n\n### 块级公式\n块级公式示例：\n\n$$ a^2 + b^2 = c^2 $$\n\n## 3. 带有代码块的示例\n```python\ndef greet(name):\n    return f\"Hello, {name}!\"\n\nprint(greet('Alice'))\n```\n\n## 4. 包含引号的文本\n这里是一个包含 `\"` 和 `'` 引号的文本。例如：\n- 双引号：`\"这是一个带有双引号的示例\"`\n- 单引号：`'这是一个带有单引号的示例'`\n",
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
