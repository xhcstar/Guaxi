import { defineStore } from "pinia";
import { createLogger } from "../utils/logger";

const logger = createLogger("rightSiderStore");
const console = {
  debug: (...args) => logger.debug(...args),
  log: (...args) => logger.info(...args),
  info: (...args) => logger.info(...args),
  warn: (...args) => logger.warn(...args),
  error: (...args) => logger.error(...args),
};

export const useRightSiderStore = defineStore("rightSider", {
  state: () => ({
    searchResults: {
      fileResults: [],
      noteResults: [],
      excerptResults: [], // 新增：摘录搜索结果
      searchQuery: "",
      lastSearchType: "file",
      error: "",
      loading: false,
    },
    // 新增：当前右侧面板类型（如 'search', 'outline' 等）
    panel: "",
    // 新增：右侧边栏是否展开
    isOpen: false,
    assistant: {
      messages: [],
      sessionId: "",
      modelId: 6,
      webSearchEnabled: true,
      pending: false,
      draftInput: "",
    },
  }),
  actions: {
    setSearchResults(results) {
      console.log("[DEBUG][Store] Setting search results:", {
        // DEBUG
        fileResults: results.fileResults?.length || 0,
        noteResults: results.noteResults?.length || 0,
        query: results.searchQuery,
        lastSearchType: results.lastSearchType,
      });

      this.searchResults = results;
    },
    clearSearchResults() {
      this.searchResults = {
        fileResults: [],
        noteResults: [],
        excerptResults: [],
        searchQuery: "",
      };
    },
    // 新增：设置当前面板类型
    setPanel(name) {
      this.panel = name;
    },
    // 新增：展开右侧边栏
    open() {
      this.isOpen = true;
    },
    // 新增：关闭右侧边栏
    close() {
      this.isOpen = false;
    },
    addAssistantMessage(message) {
      if (!message || typeof message !== "object") return;
      this.assistant.messages.push({
        id: `${Date.now()}_${Math.random().toString(36).slice(2, 8)}`,
        role: message.role || "assistant",
        content: String(message.content || ""),
        html: String(message.html || ""),
        createdAt: Date.now(),
        meta: message.meta || null,
      });
    },
    clearAssistantMessages() {
      this.assistant.messages = [];
      this.assistant.sessionId = "";
    },
    setAssistantSessionId(sessionId) {
      this.assistant.sessionId = String(sessionId || "");
    },
    setAssistantModelId(modelId) {
      const n = Number(modelId);
      if (!Number.isFinite(n)) return;
      this.assistant.modelId = n;
    },
    setAssistantWebSearchEnabled(enabled) {
      this.assistant.webSearchEnabled = !!enabled;
    },
    setAssistantPending(pending) {
      this.assistant.pending = !!pending;
    },
    appendAssistantDraft(text) {
      const value = String(text || "").trim();
      if (!value) return;
      this.assistant.draftInput = this.assistant.draftInput
        ? `${this.assistant.draftInput}\n${value}`
        : value;
    },
    consumeAssistantDraft() {
      const value = String(this.assistant.draftInput || "");
      this.assistant.draftInput = "";
      return value;
    },
  },
});
