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
      modelId: 5,
      webSearchEnabled: true,
      pending: false,
      draftInput: "",
      draftAutoSend: false,
      draftDisableWebSearch: false,
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
      /*
       * Design notes:
       * 1. The empty state of the right search panel should be defined in one place.
       * 2. Reset result lists, query, error, and loading together to avoid stale UI.
       * 3. Keep a default lastSearchType so the next search still has a valid mode.
       */
      this.searchResults = {
        fileResults: [],
        noteResults: [],
        excerptResults: [],
        searchQuery: "",
        lastSearchType: "file",
        error: "",
        loading: false,
      };
      console.info("[DEBUG][Store] Cleared right sider search results");
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
      const next = {
        id: `${Date.now()}_${Math.random().toString(36).slice(2, 8)}`,
        role: message.role || "assistant",
        content: String(message.content || ""),
        html: String(message.html || ""),
        createdAt: Date.now(),
        meta: message.meta || null,
      };
      this.assistant.messages.push(next);
      return next.id;
    },
    updateAssistantMessage(messageId, payload) {
      const id = String(messageId || "");
      if (!id || !payload || typeof payload !== "object") return;
      const index = this.assistant.messages.findIndex((x) => x.id === id);
      if (index < 0) return;
      const current = this.assistant.messages[index];
      const next = {
        ...current,
        ...payload,
      };
      this.assistant.messages.splice(index, 1, next);
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
    appendAssistantDraft(text, options) {
      const value = String(text || "").trim();
      if (!value) return;
      const autoSend = !!(options && options.autoSend);
      const disableWebSearch = !!(options && options.disableWebSearch);
      this.assistant.draftInput = this.assistant.draftInput
        ? `${this.assistant.draftInput}\n${value}`
        : value;
      this.assistant.draftAutoSend = this.assistant.draftAutoSend || autoSend;
      this.assistant.draftDisableWebSearch =
        this.assistant.draftDisableWebSearch || disableWebSearch;
    },
    consumeAssistantDraftPayload() {
      const value = String(this.assistant.draftInput || "");
      const autoSend = !!this.assistant.draftAutoSend;
      const disableWebSearch = !!this.assistant.draftDisableWebSearch;
      this.assistant.draftInput = "";
      this.assistant.draftAutoSend = false;
      this.assistant.draftDisableWebSearch = false;
      return {
        text: value,
        autoSend,
        disableWebSearch,
      };
    },
  },
});
