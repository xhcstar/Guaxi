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

const ASSISTANT_SESSIONS_STORAGE_KEY = "guaxi:assistant:sessions:v1";

function makeAssistantSession(title = "新会话", seed = {}) {
  const now = Date.now();
  return {
    key: String(seed.key || `assistant-${now}-${Math.random().toString(36).slice(2, 8)}`),
    title: String(seed.title || title || "新会话"),
    messages: Array.isArray(seed.messages) ? seed.messages : [],
    sessionId: String(seed.sessionId || ""),
    // codex Agent 的会话线程 id。codex 自己维护上下文，续问时靠它 resume 同一线程。
    codexThreadId: String(seed.codexThreadId || ""),
    createdAt: Number(seed.createdAt || now),
    updatedAt: Number(seed.updatedAt || now),
  };
}

function normalizeAssistantSession(raw) {
  const item = raw && typeof raw === "object" ? raw : {};
  return makeAssistantSession("新会话", item);
}

function readAssistantSessionsState() {
  const fallback = makeAssistantSession("新会话");
  if (typeof localStorage === "undefined") {
    return { sessions: [fallback], currentSessionKey: fallback.key };
  }

  try {
    const raw = localStorage.getItem(ASSISTANT_SESSIONS_STORAGE_KEY);
    if (!raw) return { sessions: [fallback], currentSessionKey: fallback.key };
    const parsed = JSON.parse(raw);
    const sessions = (Array.isArray(parsed?.sessions) ? parsed.sessions : [])
      .map(normalizeAssistantSession)
      .filter((item) => item.key);
    if (!sessions.length) return { sessions: [fallback], currentSessionKey: fallback.key };
    const savedCurrent = String(parsed?.currentSessionKey || "");
    const currentSessionKey = sessions.some((item) => item.key === savedCurrent)
      ? savedCurrent
      : sessions[0].key;
    return { sessions, currentSessionKey };
  } catch (error) {
    console.warn("[RightSiderStore] read assistant sessions failed", error);
    return { sessions: [fallback], currentSessionKey: fallback.key };
  }
}

function writeAssistantSessionsState(sessions, currentSessionKey) {
  if (typeof localStorage === "undefined") return;
  try {
    localStorage.setItem(
      ASSISTANT_SESSIONS_STORAGE_KEY,
      JSON.stringify({
        currentSessionKey,
        sessions: Array.isArray(sessions) ? sessions : [],
      })
    );
  } catch (error) {
    console.warn("[RightSiderStore] persist assistant sessions failed", error);
  }
}

function assistantSessionTitleFromMessages(messages) {
  const firstUser = (Array.isArray(messages) ? messages : [])
    .find((item) => item?.role === "user" && String(item?.content || "").trim());
  const text = String(firstUser?.content || "").trim().replace(/\s+/g, " ");
  return text ? text.slice(0, 24) : "新会话";
}

export const useRightSiderStore = defineStore("rightSider", {
  state: () => {
    const assistantSessionsState = readAssistantSessionsState();
    const currentSession = assistantSessionsState.sessions.find((item) => item.key === assistantSessionsState.currentSessionKey)
      || assistantSessionsState.sessions[0];
    return {
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
        messages: currentSession.messages,
        sessionId: currentSession.sessionId,
        codexThreadId: currentSession.codexThreadId,
        sessions: assistantSessionsState.sessions,
        currentSessionKey: currentSession.key,
        modelId: 5,
        task: "chat",
        webSearchEnabled: true,
        pending: false,
        draftInput: "",
        draftAutoSend: false,
        draftDisableWebSearch: false,
      },
      pythonSessionInspector: {
        filePath: "",
        cellId: "",
        cellKey: "",
        stableId: "",
        cellTitle: "",
        sessionId: "",
        hasActiveCell: false,
        datasets: [],
        variables: [],
        updatedAt: 0,
      },
    };
  },
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
      console.info("[RightSiderStore] set panel", { panel: name });
    },
    // 新增：展开右侧边栏
    open() {
      this.isOpen = true;
      console.info("[RightSiderStore] open right sider", { panel: this.panel });
    },
    // 新增：关闭右侧边栏
    close() {
      this.isOpen = false;
      console.info("[RightSiderStore] close right sider", { panel: this.panel });
    },
    persistAssistantSessions() {
      writeAssistantSessionsState(this.assistant.sessions, this.assistant.currentSessionKey);
    },
    getCurrentAssistantSession() {
      const key = String(this.assistant.currentSessionKey || "");
      let session = this.assistant.sessions.find((item) => item.key === key);
      if (!session) {
        session = makeAssistantSession("新会话");
        this.assistant.sessions.push(session);
        this.assistant.currentSessionKey = session.key;
      }
      return session;
    },
    syncCurrentAssistantSession() {
      const session = this.getCurrentAssistantSession();
      session.messages = Array.isArray(this.assistant.messages) ? this.assistant.messages : [];
      session.sessionId = String(this.assistant.sessionId || "");
      session.codexThreadId = String(this.assistant.codexThreadId || "");
      session.updatedAt = Date.now();
      if (!session.title || session.title === "新会话") {
        session.title = assistantSessionTitleFromMessages(session.messages);
      }
      this.persistAssistantSessions();
      console.debug("[RightSiderStore] synced assistant session", {
        key: session.key,
        title: session.title,
        messageCount: session.messages.length,
      });
    },
    createAssistantSession(title = "新会话") {
      this.syncCurrentAssistantSession();
      const session = makeAssistantSession(title);
      this.assistant.sessions.unshift(session);
      this.assistant.currentSessionKey = session.key;
      this.assistant.messages = session.messages;
      this.assistant.sessionId = "";
      this.assistant.codexThreadId = "";
      this.persistAssistantSessions();
      console.info("[RightSiderStore] created assistant session", { key: session.key });
      return session.key;
    },
    switchAssistantSession(key) {
      const targetKey = String(key || "");
      const session = this.assistant.sessions.find((item) => item.key === targetKey);
      if (!session) return false;
      this.syncCurrentAssistantSession();
      this.assistant.currentSessionKey = session.key;
      this.assistant.messages = session.messages;
      this.assistant.sessionId = String(session.sessionId || "");
      this.assistant.codexThreadId = String(session.codexThreadId || "");
      this.persistAssistantSessions();
      console.info("[RightSiderStore] switched assistant session", {
        key: session.key,
        messageCount: session.messages.length,
      });
      return true;
    },
    deleteAssistantSession(key) {
      const targetKey = String(key || this.assistant.currentSessionKey || "");
      const index = this.assistant.sessions.findIndex((item) => item.key === targetKey);
      if (index < 0) return false;
      const removed = this.assistant.sessions.splice(index, 1)[0];
      if (!this.assistant.sessions.length) {
        this.assistant.sessions.push(makeAssistantSession("新会话"));
      }
      const next = this.assistant.sessions[Math.min(index, this.assistant.sessions.length - 1)];
      this.assistant.currentSessionKey = next.key;
      this.assistant.messages = next.messages;
      this.assistant.sessionId = String(next.sessionId || "");
      this.assistant.codexThreadId = String(next.codexThreadId || "");
      this.persistAssistantSessions();
      console.info("[RightSiderStore] deleted assistant session", { key: removed.key, nextKey: next.key });
      return true;
    },
    removeAssistantMessages(messageIds) {
      const ids = new Set((Array.isArray(messageIds) ? messageIds : []).map((id) => String(id || "")));
      if (!ids.size) return 0;
      const before = this.assistant.messages.length;
      this.assistant.messages = this.assistant.messages.filter((item) => !ids.has(String(item?.id || "")));
      const removed = before - this.assistant.messages.length;
      this.syncCurrentAssistantSession();
      console.info("[RightSiderStore] removed assistant messages", { removed });
      return removed;
    },
    addAssistantMessage(message) {
      if (!message || typeof message !== "object") return;
      const next = Object.assign({
        id: `${Date.now()}_${Math.random().toString(36).slice(2, 8)}`,
        role: message.role || "assistant",
        content: String(message.content || ""),
        html: String(message.html || ""),
        createdAt: Date.now(),
        meta: message.meta || null,
      }, message);
      this.assistant.messages.push(next);
      this.syncCurrentAssistantSession();
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
      this.syncCurrentAssistantSession();
    },
    clearAssistantMessages() {
      this.assistant.messages = [];
      this.assistant.sessionId = "";
      // 清空消息等于开一段新对话，codex 线程也必须丢弃，否则续问会带上已经看不见的上文。
      this.assistant.codexThreadId = "";
      this.syncCurrentAssistantSession();
    },
    setAssistantSessionId(sessionId) {
      this.assistant.sessionId = String(sessionId || "");
      this.syncCurrentAssistantSession();
    },
    setAssistantCodexThreadId(threadId) {
      const next = String(threadId || "");
      if (next === String(this.assistant.codexThreadId || "")) return;
      this.assistant.codexThreadId = next;
      this.syncCurrentAssistantSession();
      console.info("[RightSiderStore] assistant codex thread updated", { threadId: next });
    },
    setAssistantModelId(modelId) {
      const n = Number(modelId);
      if (!Number.isFinite(n)) return;
      this.assistant.modelId = n;
    },
    setAssistantTask(task) {
      const allowed = ["chat", "write", "translate", "organize", "classify", "tag", "agent"];
      const next = String(task || "chat");
      this.assistant.task = allowed.includes(next) ? next : "chat";
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
    setPythonSessionInspector(payload) {
      const next = payload && typeof payload === "object" ? payload : {};
      this.pythonSessionInspector = {
        filePath: String(next.filePath || ""),
        cellId: String(next.cellId || ""),
        cellKey: String(next.cellKey || ""),
        stableId: String(next.stableId || ""),
        cellTitle: String(next.cellTitle || ""),
        sessionId: String(next.sessionId || ""),
        hasActiveCell: !!next.hasActiveCell,
        datasets: Array.isArray(next.datasets) ? next.datasets : [],
        variables: Array.isArray(next.variables) ? next.variables : [],
        updatedAt: Date.now(),
      };
      console.debug("[RightSiderStore] sync python session inspector", {
        cellId: this.pythonSessionInspector.cellId,
        hasActiveCell: this.pythonSessionInspector.hasActiveCell,
        datasetCount: this.pythonSessionInspector.datasets.length,
        variableCount: this.pythonSessionInspector.variables.length,
      });
    },
    clearPythonSessionInspector() {
      this.pythonSessionInspector = {
        filePath: "",
        cellId: "",
        cellKey: "",
        stableId: "",
        cellTitle: "",
        sessionId: "",
        hasActiveCell: false,
        datasets: [],
        variables: [],
        updatedAt: Date.now(),
      };
      console.info("[RightSiderStore] cleared python session inspector");
    },
  },
});
