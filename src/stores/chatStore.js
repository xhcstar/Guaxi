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
          //Messages1
          {
            topicid: 0,
            topic: [
              {
                msgid: 1,
                sender: "ChatGPT",
                chatSessionId: "",
                content: "你好，有什么可以帮忙的吗？",
              },
              {
                msgid: 2,
                sender: "me",
                chatSessionId: "",
                content: "我叫瓜西",
              },
              {
                msgid: 3,
                sender: "ChatGPT",
                chatSessionId: "",
                content: "你为什叫瓜西？",
              },
              {
                msgid: 4,
                sender: "me",
                chatSessionId: "",
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
                chatSessionId: "",
                content: "你好, 我是ChatGPT, 有什么可以帮忙的吗？",
              },
              {
                msgid: 2,
                sender: "me",
                chatSessionId: "",
                content: "我叫Siri",
              },
              {
                msgid: 3,
                sender: "ChatGPT",
                chatSessionId: "",
                content: "你为什叫Siri?",
              },
              {
                msgid: 4,
                sender: "me",
                chatSessionId: "",
                content: "你管！",
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
          //Messages1
          {
            topicid: 0,
            topic: [
              {
                msgid: 1,
                sender: "DeepSeek",
                chatSessionId: "",
                content: "你好，有什么可以帮忙的吗？",
              },
              {
                msgid: 2,
                sender: "me",
                chatSessionId: "",
                content: "我叫瓜西",
              },
              {
                msgid: 3,
                sender: "DeepSeek",
                chatSessionId: "",
                content:
                  '# 示例标题\n\n这是一个包含 **Markdown**、数学公式和代码块的大段文本。以下是几种类型的内容：\n\n## 1. Markdown 格式\nMarkdown 格式可以使用很多常见的语法。比如：\n- **加粗**，*斜体*，~~删除线~~\n- 列表：\n  - 第一项\n  - 第二项\n  - 第三项\n- 链接：[Google](https://www.google.com)\n\n### 代码块（不转义引号）\n以下是一个示例代码块，它展示了如何显示引号并保持格式不变：\n\n```javascript\nconst message = "Hello, world!";\nconsole.log(message);  // 输出: Hello, world!\n```\n\n## 2. 数学公式\nMarkdown 也支持数学公式。你可以使用 `$$` 来写块级公式，或者使用 `$` 来写行内公式。\n\n### 行内公式\n行内公式示例：$E = mc^2$\n\n### 块级公式\n块级公式示例：\n\n$$ a^2 + b^2 = c^2 $$\n\n## 3. 带有代码块的示例\n```python\ndef greet(name):\n    return f"Hello, {name}!"\n\nprint(greet(\'Alice\'))\n```\n\n## 4. 包含引号的文本\n这里是一个包含 `"` 和 `\'` 引号的文本。例如：\n- 双引号：`"这是一个带有双引号的示例"`\n- 单引号：`\'这是一个带有单引号的示例\'`\n',
              },
              {
                msgid: 4,
                sender: "me",
                chatSessionId: "",
                content: "你管！",
              },
              {
                msgid: 5,
                sender: "DeepSeek",
                chatSessionId: "",
                content: `"麦克斯韦方程组（Maxwell's Equations）是描述电磁场行为的一组基本方程，由19世纪物理学家詹姆斯·克拉克·麦克斯韦（James Clerk Maxwell）提出。这组方程统一了电学和磁学，并预言了电磁波的存在，为现代电磁学奠定了基础。麦克斯韦方程组包括四个方程，通常以微分形式或积分形式表示。\n\n### 麦克斯韦方程组的微分形式\n1. **高斯定律（Gauss's Law for Electricity）**  \n   \\[\n   \\nabla \\cdot \\mathbf{E} = \\frac{\\rho}{\\varepsilon_0}\n   \\]  \n   描述电场 \\(\\mathbf{E}\\) 与电荷密度 \\(\\rho\\) 的关系，表明电场是由电荷产生的。\n\n2. **高斯磁定律（Gauss's Law for Magnetism）**  \n   \\[\n   \\nabla \\cdot \\mathbf{B} = 0\n   \\]  \n   描述磁场 \\(\\mathbf{B}\\) 的性质，表明磁场是无源的，即不存在磁单极子。\n\n3. **法拉第电磁感应定律（Faraday's Law of Induction）**  \n   \\[\n   \\nabla \\times \\mathbf{E} = -\\frac{\\partial \\mathbf{B}}{\\partial t}\n   \\]  \n   描述变化的磁场如何产生电场。\n\n4. **安培-麦克斯韦定律（Ampère's Law with Maxwell's Addition）**  \n   \\[\n   \\nabla \\times \\mathbf{B} = \\mu_0 \\mathbf{J} + \\mu_0 \\varepsilon_0 \\frac{\\partial \\mathbf{E}}{\\partial t}\n   \\]  \n   描述电流 \\(\\mathbf{J}\\) 和变化的电场如何产生磁场。\n\n### 麦克斯韦方程组的积分形式\n1. **高斯定律**  \n   \\[\n   \\oint_{\\partial V} \\mathbf{E} \\cdot d\\mathbf{A} = \\frac{Q_{\\text{enc}}}{\\varepsilon_0}\n   \\]  \n   电场通过闭合曲面的通量等于该曲面内包围的总电荷除以真空电容率 \\(\\varepsilon_0\\)。\n\n2. **高斯磁定律**  \n   \\[\n   \\oint_{\\partial V} \\mathbf{B} \\cdot d\\mathbf{A} = 0\n   \\]  \n   磁场通过任何闭合曲面的通量为零。\n\n3. **法拉第电磁感应定律**  \n   \\[\n   \\oint_{\\partial S} \\mathbf{E} \\cdot d\\mathbf{l} = -\\frac{d}{dt} \\int_S \\mathbf{B} \\cdot d\\mathbf{A}\n   \\]  \n   电场沿闭合路径的环流等于通过该路径包围曲面的磁通量的变化率的负值。\n\n4. **安培-麦克斯韦定律**  \n   \\[\n   \\oint_{\\partial S} \\mathbf{B} \\cdot d\\mathbf{l} = \\mu_0 I_{\\text{enc}} + \\mu_0 \\varepsilon_0 \\frac{d}{dt} \\int_S \\mathbf{E} \\cdot d\\mathbf{A}\n   \\]  \n   磁场沿闭合路径的环流等于通过该路径包围曲面的电流加上电场通量的变化率。\n\n### 物理意义\n- 麦克斯韦方程组揭示了电场和磁场的 相互联系，表明变化的电场可以产生磁场，变化的磁场可以产生电场。\n- 这组方程预言了电磁波的存在，并表明光是一种电磁波。\n- 麦克斯韦方程组是经典电磁学的核心，也是现代物理学和工程学的基础。\n\n### 符号说明\n- \\(\\mathbf{E}\\)：电场强度（单位：V/m）\n- \\(\\mathbf{B}\\)：磁感应强度（单位：T）\n- \\(\\rho\\)：电荷密度（单位：C/m³）\n- \\(\\mathbf{J}\\)：电流密度（单位：A/m²）\n- \\(\\varepsilon_0\\)：真空电容率（单位：F/m）\n- \\(\\mu_0\\)：真空磁导率（单位：H/m）\n- \\(Q_{\\text{enc}}\\)：闭合曲面内包围的总电荷\n- \\(I_{\\text{enc}}\\)：闭合路径内包围的总电流\n\n麦克斯韦方程组是物理学中最伟大的成就之一，它不仅统一了电学和磁学，还为后来的相对论和量子力学的发展提供了重要启示。"`,
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
                chatSessionId: "",
                content: "你好, 我是DeepSeek, 有什么可以帮忙的吗？",
              },
              {
                msgid: 2,
                sender: "me",
                chatSessionId: "",
                content: "我叫瓜西",
              },
              {
                msgid: 3,
                sender: "DeepSeek",
                chatSessionId: "",
                content: "你为什叫瓜西？",
              },
              {
                msgid: 4,
                sender: "me",
                chatSessionId: "",
                content: "你管！",
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
          //Messages1
          {
            topicid: 0,
            topic: [
              {
                msgid: 1,
                sender: "Qwen",
                chatSessionId: "",
                content: "你好, 我是Qwen, 有什么可以帮忙的吗？",
              },
              {
                msgid: 2,
                sender: "me",
                chatSessionId: "",
                content: "我叫瓜西",
              },
              {
                msgid: 3,
                sender: "Qwen",
                chatSessionId: "",
                content: "你为什叫瓜西？",
              },
              {
                msgid: 4,
                sender: "me",
                chatSessionId: "",
                content: "你管！",
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
          //Messages1
          {
            topicid: 0,
            topic: [
              {
                msgid: 1,
                sender: "Kimi",
                chatSessionId: "",
                content: "你好, 我是Kimi, 有什么可以帮忙的吗？",
              },
              {
                msgid: 2,
                sender: "me",
                chatSessionId: "",
                content: "我叫瓜西",
              },
              {
                msgid: 3,
                sender: "Kimi",
                chatSessionId: "",
                content: "你为什叫瓜西？",
              },
              {
                msgid: 4,
                sender: "me",
                chatSessionId: "",
                content: "你管！",
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
          //Messages1
          {
            topicid: 0,
            topic: [
              {
                msgid: 1,
                sender: "豆包",
                chatSessionId: "",
                content: "你好, 我是豆包, 有什么可以帮忙的吗？",
              },
              {
                msgid: 2,
                sender: "me",
                chatSessionId: "",
                content: "我叫瓜西",
              },
              {
                msgid: 3,
                sender: "豆包",
                chatSessionId: "",
                content: "你为什叫瓜西？",
              },
              {
                msgid: 4,
                sender: "me",
                chatSessionId: "",
                content: "你管！",
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
          //Messages1
          {
            topicid: 0,
            topic: [
              {
                msgid: 1,
                sender: "Local-Qwen",
                chatSessionId: "",
                content: "你好，有什么可以帮忙的吗？",
              },
              {
                msgid: 2,
                sender: "me",
                chatSessionId: "",
                content: "我叫瓜西",
              },
              {
                msgid: 3,
                sender: "Local-Qwen",
                chatSessionId: "",
                content: "你为什叫瓜西？",
              },
              {
                msgid: 4,
                sender: "me",
                chatSessionId: "",
                content: "你管！",
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
          //Messages1
          {
            topicid: 0,
            topic: [
              {
                msgid: 1,
                sender: "Local-DeepSeek",
                chatSessionId: "",
                content: "你好，有什么可以帮忙的吗？",
              },
              {
                msgid: 2,
                sender: "me",
                chatSessionId: "",
                content: "我叫瓜西",
              },
              {
                msgid: 3,
                sender: "Local-DeepSeek",
                chatSessionId: "",
                content: "你为什叫瓜西？",
              },
              {
                msgid: 4,
                sender: "me",
                chatSessionId: "",
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
                answer = "";
              } else {
                // 如果没有消息条目，就创建一个新的
                topicitem.topic.push({
                  msgid: topicitem.topic.length,
                  sender: session.name,
                  content: answer,
                });
              }
            }
          });
        }
      });
    },

    addChatSession(newSession) {
      this.SelectModelchatSessions.push(newSession);
    },
  },
});
