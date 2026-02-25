import { defineStore } from "pinia";
import { invoke } from "@tauri-apps/api/core";
import { createLogger } from "../utils/logger";

const logger = createLogger("knowledgeBaseStore");
const console = {
  debug: (...args) => logger.debug(...args),
  log: (...args) => logger.info(...args),
  info: (...args) => logger.info(...args),
  warn: (...args) => logger.warn(...args),
  error: (...args) => logger.error(...args),
};

const normalizeGraphNodeType = (rawType) => {
  const t = String(rawType || "").trim().toLowerCase();
  const map = {
    "knowledge-base": "knowledge-base",
    knowledge_base: "knowledge-base",
    kb: "knowledge-base",
    directory: "directory",
    dir: "directory",
    folder: "directory",
    file: "file",
    section: "section",
    heading: "section",
    header: "section",
    title: "section",
    paragraph: "paragraph",
    para: "paragraph",
    content: "paragraph",
    text: "paragraph",
  };
  return map[t] || "unknown";
};

export const useKnowledgeBaseStore = defineStore("knowledgeBase", {
  state: () => ({
    knowledgeBases: [], // 知识库列表
    loading: false, // 加载状态
    showForm: false, // 是否显示表单
    selectedKnowledgeBase: null, // 当前选中的知识库
    formData: {
      // 表单数据
      knowledgeBaseName: "", // 知识库名称
      knowledgeBaseTags: "", // 知识库标签
      knowledgeBaseDesc: "", // 知识库描述
    },

    selectedTreeNodeKey: null,
    focusTarget: null,

    graphNodes: [],
    graphEdges: [],
    tagOptions: [],

    searchQuery: "",
    visibleTypes: ["knowledge-base", "directory", "file", "section", "paragraph"],
    selectedTags: [],

    selectedGraphNode: null,
  }),

  actions: {
    /**
     * 获取知识库图形数据
     * @param {string} knowledgeBaseId - 知识库ID
     * @returns {Promise<Object>} 图形数据
     */
    async fetchGraphData(knowledgeBaseId) {
      this.loading = true;
      try {
        const graphData = await invoke("get_knowledge_graph_data", {
          knowledgeBaseId: parseInt(knowledgeBaseId), // 确保传递数字类型
        });

        console.log("获取图形数据成功:", graphData);

        const out = {
          nodes: graphData.nodes.map((node) => ({
            ...node,
            type: normalizeGraphNodeType(node.type),
          })),
          edges: graphData.edges.map((edge) => ({
            ...edge,
            type: edge.type || "relation",
          })),
        };

        this.setGraphIndex(out);
        return out;
      } catch (error) {
        console.error("获取图形数据失败:", error);
        throw error;
      } finally {
        this.loading = false;
      }
    },

    /**
     * 获取知识库列表（自动选中第一个）
     */
    async fetchKnowledgeBases() {
      this.loading = true;
      try {
        const list = await invoke("get_knowledge_base_list");

        // 标准化所有路径
        this.knowledgeBases = list.map((kb) => ({
          ...kb,
          location: kb.location.replace(/\\/g, "/"),
        }));

        // 自动选中第一个知识库（如果存在且当前未选中）
        if (this.knowledgeBases.length > 0 && !this.selectedKnowledgeBase) {
          this.setSelectedKnowledgeBase(this.knowledgeBases[0]);
        }

        return this.knowledgeBases;
      } catch (error) {
        console.error("获取知识库列表失败:", error);
        throw error;
      } finally {
        this.loading = false;
      }
    },

    /**
     * 设置当前选中的知识库
     */
    setSelectedKnowledgeBase(kb) {
      if (!kb) {
        this.selectedKnowledgeBase = null;
        this.selectedTreeNodeKey = null;
        this.focusTarget = null;
        this.graphNodes = [];
        this.graphEdges = [];
        this.tagOptions = [];
        this.searchQuery = "";
        this.selectedTags = [];
        this.selectedGraphNode = null;
        return;
      }

      this.selectedKnowledgeBase = {
        ...kb,
        location: kb.location.replace(/\\/g, "/"),
      };

      this.selectedTreeNodeKey = null;
      this.focusTarget = null;
      this.searchQuery = "";
      this.selectedTags = [];
      this.selectedGraphNode = null;
    },

    setSelectedGraphNode(node) {
      this.selectedGraphNode = node || null;
    },

    /**
     * 创建知识库
     * @throws {Error} 创建失败时抛出错误
     */
    async createKnowledgeBase() {
      // 验证知识库名称
      if (!this.formData.knowledgeBaseName.trim()) {
        throw new Error("知识库名字不能为空");
      }

      try {
        // 调用 Tauri 命令创建知识库
        await invoke("create_knowledge_base", {
          input: {
            knowledge_base_name: this.formData.knowledgeBaseName.trim(),
            knowledge_base_tags: this.formData.knowledgeBaseTags.trim(),
            knowledge_base_description: this.formData.knowledgeBaseDesc.trim(),
          },
        });

        // 重置表单并刷新列表
        this.resetForm();
        await this.fetchKnowledgeBases();
        return true; // 表示创建成功
      } catch (error) {
        console.error("创建知识库失败:", error);
        throw error; // 抛出错误以便组件处理
      }
    },

    /**
     * 重置表单数据
     */
    resetForm() {
      this.formData = {
        knowledgeBaseName: "",
        knowledgeBaseTags: "",
        knowledgeBaseDesc: "",
      };
    },

    /**
     * 切换表单显示状态
     * @param {boolean} [show] - 可选，指定显示或隐藏
     */
    toggleForm(show) {
      this.showForm = show ?? !this.showForm;
    },

    setSelectedTreeNodeKey(key) {
      const v = key == null ? null : String(key).replace(/\\/g, "/");
      this.selectedTreeNodeKey = v;
      if (v) {
        this.requestFocus({ mode: "path", value: v });
      }
    },

    requestFocus(target) {
      if (!target || !target.value) return;
      this.focusTarget = {
        mode: target.mode || "id",
        value: String(target.value),
        ts: Date.now(),
      };
    },

    setSearchQuery(v) {
      this.searchQuery = String(v || "");
    },

    setVisibleTypes(types) {
      this.visibleTypes = Array.isArray(types) ? types.map(String) : [];
    },

    setSelectedTags(tags) {
      this.selectedTags = Array.isArray(tags) ? tags.map(String) : [];
    },

    setGraphIndex(graphData) {
      const nodes = Array.isArray(graphData?.nodes) ? graphData.nodes : [];
      const edges = Array.isArray(graphData?.edges) ? graphData.edges : [];

      this.graphNodes = nodes;
      this.graphEdges = edges;

      const tagSet = new Set();
      for (const n of nodes) {
        const ts = n?.tags;
        if (!Array.isArray(ts)) continue;
        for (const t of ts) {
          const s = String(t || "").trim();
          if (s) tagSet.add(s);
        }
      }

      this.tagOptions = Array.from(tagSet)
        .sort((a, b) => a.localeCompare(b, "zh"))
        .map((t) => ({ label: t, value: t }));
    },
  },
});
