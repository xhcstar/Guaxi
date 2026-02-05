import { defineStore } from "pinia";
import { renderMarkdown } from "../components/GuaxiMarkdownRender/GuaxiMarker";
import { createLogger } from "../utils/logger";
import chatgptImg from '@/assets/chatgpt.png';
import deepseekImg from '@/assets/deepseek.png';
import qwenImg from '@/assets/qwen.png';
import yuanbao from '@/assets/yuanbao.png';
import kimiImg from '@/assets/kimi.png';
import doubaoImg from '@/assets/doubao.png';
import localQwenImg from '@/assets/local_qwen.png';
import localDeepseekImg from '@/assets/local_deepseek.png';
import meImg from '@/assets/me.png';

const logger = createLogger("chatStore");
const console = {
  debug: (...args) => logger.debug(...args),
  log: (...args) => logger.info(...args),
  info: (...args) => logger.info(...args),
  warn: (...args) => logger.warn(...args),
  error: (...args) => logger.error(...args),
};

export const useChatStore = defineStore("chatStore", {
  state: () => ({
    SelectModelchatSessions: [
      {
        id: 0,
        name: "ChatGPT",
        ModelAvatar: chatgptImg,
        MeAvatar: meImg,
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
        ModelAvatar: deepseekImg,
        MeAvatar: meImg,
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
        ModelAvatar: qwenImg,
        MeAvatar: meImg,
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
        ModelAvatar: kimiImg,
        MeAvatar: meImg,
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
        ModelAvatar: doubaoImg,
        MeAvatar: meImg,
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
        ModelAvatar: localQwenImg,
        MeAvatar: meImg,
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
        ModelAvatar: localDeepseekImg,
        MeAvatar: meImg,
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

      {
        id: 7,
        name: "腾讯元宝(网页)",
        ModelAvatar: yuanbao,
        MeAvatar: meImg,
        MessagesList: [
          {
            topicid: 0,
            topic: [
              {
                msgid: 0,
                sender: "腾讯元宝(网页)",
                chatSessionId: "",
                content: "您好，我是腾讯元宝，有什么可以帮忙的吗？",
              },
            ],
          },
        ],
      },

      {
        id: 8,
        name: "ChatGPT(网页)",
        ModelAvatar: chatgptImg,
        MeAvatar: meImg,
        MessagesList: [
          {
            topicid: 0,
            topic: [
              {
                msgid: 0,
                sender: "ChatGPT(网页)",
                chatSessionId: "",
                content: "您好，我是 ChatGPT，有什么可以帮忙的吗？",
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
        { name: "腾讯元宝(网页)", selectedModel: "7" },
        { name: "ChatGPT(网页)", selectedModel: "8" },
      ],

      localModel: [
        { name: "Local-Qwen", selectedModel: "5" },
        { name: "Local-DeepSeek", selectedModel: "6" },
      ],
    },

    lastMessage: null,
    lastMessageTime: 0,
    accumulatedMessage: "",

    streamTargetModelId: null,
    streamTargetTopicId: null,
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

    setStreamTarget(modelId, topicId) {
      this.streamTargetModelId = modelId;
      this.streamTargetTopicId = topicId;
      this.lastMessage = null;
      this.lastMessageTime = 0;
    },

    appendMessagesToTopic({ modelId, topicId, sessionId, title, messages }) {
      const targetModelId = typeof modelId === 'number' ? modelId : this.selectedModel;
      const targetTopicId = typeof topicId === 'number' ? topicId : this.selectedTopic;
      const safeMessages = Array.isArray(messages) ? messages : [];
      const safeTitle = (title ?? '').toString().trim();

      this.SelectModelchatSessions.find((session) => {
        if (session.id !== targetModelId) return;

        let topicitem = session.MessagesList.find((t) => t.topicid === targetTopicId);
        if (!topicitem) {
          topicitem = {
            topicid: targetTopicId,
            title: '',
            topic: [],
          };
          session.MessagesList.push(topicitem);
        }

        if (safeTitle) {
          topicitem.title = safeTitle;
        }

        safeMessages.forEach((m) => {
          const role = (m?.role || '').toString().toLowerCase();
          const sender = role === 'user' ? 'me' : session.name;
          const raw = (m?.content ?? '').toString();
          const messageId = (m?.message_id ?? m?.messageId ?? '').toString();

          const applyContent = (target) => {
            target.raw = raw;
            if (sender === 'me') {
              target.content = plainToHtml(raw);
              return;
            }
          };

          if (messageId) {
            const existing = topicitem.topic.find((x) => x?.messageId === messageId && x?.sender === sender);
            if (existing) {
              existing.chatSessionId = sessionId || existing.chatSessionId || '';
              applyContent(existing);
              return;
            }
          }

          const next = {
            msgid: topicitem.topic.length + 1,
            sender,
            chatSessionId: sessionId || '',
            raw,
            content: raw,
            messageId: messageId || '',
          };

          applyContent(next);
          topicitem.topic.push(next);
        });

        if (sessionId && targetModelId === this.selectedModel && targetTopicId === this.selectedTopic) {
          this.sendChatSessionId = sessionId;
        }
      });
    },

    updateChatSession(answer) {
      // console.log("answer", answer);
      // console.log("this.selectedModel", this.selectedModel);
      // console.log("this.selectedTopic", this.selectedTopic);
      this.SelectModelchatSessions.find((session) => {
        // console.log("session", session);
        if (session.id === this.selectedModel) {
          session.MessagesList.find((topicitem) => {
            if (topicitem.topicid === this.selectedTopic) {
              // console.log("topicitem", topicitem);
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
    collectChatSessionStream(message) {
      const now = Date.now();

      if (message === this.lastMessage && now - this.lastMessageTime < 100) {
        return;
      }
      // console.log("message", message);

      this.lastMessage = message;
      this.lastMessageTime = now;

      if (typeof message === "string" && message.length > 0) {
        const processedMessage = message
          .replace(/(\s*)\$(\s*)/g, "$")
          .replace(/(\s*)\$\$(\s*)/g, "$$");

        this.accumulatedMessage += processedMessage;
        this.updateMessageContent();
      }
    },

    updateMessageContent() {
      const modelId = this.streamTargetModelId ?? this.selectedModel;
      const topicId = this.streamTargetTopicId ?? this.selectedTopic;

      this.SelectModelchatSessions.find((session) => {
        if (session.id === modelId) {
          session.MessagesList.find((topicitem) => {
            if (topicitem.topicid === topicId) {
              const latestTopic = topicitem.topic[topicitem.topic.length - 1];
              if (latestTopic && latestTopic.sender !== "me") {
                this.renderDebounced(this.accumulatedMessage).then(
                  (renderedMessage) => {
                    latestTopic.content = renderedMessage;
                  }
                );
              } else {
                topicitem.topic.push({
                  msgid: topicitem.topic.length,
                  sender: session.name,
                  content: this.accumulatedMessage,
                });
              }
            }
          });
        }
      });
    },

    renderDebounced(content) {
      const text = (content ?? '').toString();

      if (!this.__renderDebounceState) {
        this.__renderDebounceState = {
          timer: null,
          pending: [],
          last: '',
        };
      }

      const state = this.__renderDebounceState;
      state.last = text;

      return new Promise((resolve) => {
        state.pending.push(resolve);

        if (state.timer) {
          clearTimeout(state.timer);
        }

        state.timer = setTimeout(async () => {
          state.timer = null;

          const pending = state.pending.splice(0, state.pending.length);
          const toRender = state.last;

          let rendered = '';
          try {
            console.log('toRender', toRender);
            rendered = await renderMarkdown(toRender, {
              basePath: "",
              mermaid: true,
              math: true,
              codeHighlight: true,
              theme: "default",
            });
          } catch (error) {
            console.error("渲染错误:", error);
            rendered = `<div class="render-error">${toRender}</div>`;
          }

          for (const r of pending) {
            try { r(rendered); } catch (e) { }
          }
        }, 50);
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
                content: `您好，我是${session.name}, 有什么可以帮忙的吗？`,
              },
            ],
          });
        }
      });
    },
  },
});
