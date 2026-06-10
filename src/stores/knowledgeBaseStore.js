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
    graphMeta: { totalNodes: 0, returnedNodes: 0, truncated: false },
    tagOptions: [],

    searchQuery: "",
    visibleTypes: ["knowledge-base", "directory", "file", "section", "paragraph"],
    selectedTags: [],

    selectedGraphNode: null,

    // 图谱视图模式:classic(经典) | kg3d(工业,默认)
    graphViewMode: "kg3d",

    // ─── 工业图谱(kg3d)与左右边栏共享的通信总线 ───
    // kg3d 组件把当前选中节点发布到这里,供右侧边栏渲染操作面板;
    // 边栏通过 requestKg3dAction 反向驱动 kg3d 做视图动作(聚焦/打开/刷新等)。
    kg3dSelected: null,
    kg3dActionRequest: null, // { type, payload, ts }
    kg3dRelations: [], // 当前选中节点已有关系(由 kg3d 组件发布,供右栏渲染)
    kg3dSuggestions: [], // 当前关联建议
    kg3dHistory: [], // 最近访问节点 [{id,name,type,ts}]
    kg3dBookmarks: [], // 收藏节点 [{id,name,type}]
  }),

  actions: {
    /**
     * 获取知识库图形数据
     * @param {string} knowledgeBaseId - 知识库ID
     * @returns {Promise<Object>} 图形数据
     */
    async fetchGraphData(knowledgeBaseId, opts = null) {
      this.loading = true;
      try {
        // 解析知识库名(新图索引以 kb_name 为键)
        const kb =
          this.knowledgeBases.find((k) => String(k.id) === String(knowledgeBaseId)) ||
          this.selectedKnowledgeBase;
        const kbName = kb?.knowledge_base_name;

        // 优先走固化的图索引(kg_query_graph),失败再回退旧的现场遍历命令。
        if (kbName) {
          try {
            const kgOpts = opts || { max_depth: 99, limit: 100000 };
            const kg = await invoke("kg_query_graph", { kbName, opts: kgOpts });
            const out = {
              nodes: kg.nodes.map((node) => ({
                ...node,
                type: normalizeGraphNodeType(node.type),
              })),
              edges: kg.edges.map((edge) => ({
                ...edge,
                type: edge.type || "relation",
              })),
            };
            console.log(
              `图索引数据: 总节点=${kg.total_nodes} 返回=${kg.returned_nodes} 截断=${kg.truncated}`
            );
            this.graphMeta = {
              totalNodes: kg.total_nodes,
              returnedNodes: kg.returned_nodes,
              truncated: kg.truncated,
            };
            this.setGraphIndex(out);
            return out;
          } catch (kgErr) {
            console.warn("kg_query_graph 失败,回退旧命令:", kgErr);
          }
        }

        const graphData = await invoke("get_knowledge_graph_data", {
          knowledgeBaseId: parseInt(knowledgeBaseId), // 确保传递数字类型
        });

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
    // ─────────── 知识图谱(kgraph)新能力封装 ───────────

    async kgEnsureIndex(kbName) {
      const name = kbName || this.selectedKnowledgeBase?.knowledge_base_name;
      if (!name) throw new Error("未选择知识库");
      return await invoke("kg_build_index", { kbName: name });
    },
    async kgIndexStatus(kbName) {
      const name = kbName || this.selectedKnowledgeBase?.knowledge_base_name;
      if (!name) return null;
      return await invoke("kg_index_status", { kbName: name });
    },
    async kgQueryGraph(kbName, opts = null) {
      const name = kbName || this.selectedKnowledgeBase?.knowledge_base_name;
      if (!name) throw new Error("未选择知识库");
      return await invoke("kg_query_graph", { kbName: name, opts });
    },
    async kgNeighbors(kbName, nodeId, hops = 1, limit = 1500, edgeTypes = null) {
      const name = kbName || this.selectedKnowledgeBase?.knowledge_base_name;
      return await invoke("kg_node_neighbors", { kbName: name, nodeId, hops, limit, edgeTypes });
    },
    async kgShortestPath(kbName, fromId, toId) {
      const name = kbName || this.selectedKnowledgeBase?.knowledge_base_name;
      return await invoke("kg_shortest_path", { kbName: name, fromId, toId });
    },
    async kgStats(kbName, topN = 20) {
      const name = kbName || this.selectedKnowledgeBase?.knowledge_base_name;
      return await invoke("kg_stats", { kbName: name, topN });
    },
    async kgCommunities(kbName) {
      const name = kbName || this.selectedKnowledgeBase?.knowledge_base_name;
      return await invoke("kg_communities", { kbName: name });
    },
    async kgDetectCommunities(kbName, resolution = 1.0) {
      const name = kbName || this.selectedKnowledgeBase?.knowledge_base_name;
      return await invoke("kg_detect_communities", { kbName: name, resolution });
    },
    async kgSemanticSearch(kbName, query, topK = 20, nodeTypes = null) {
      const name = kbName || this.selectedKnowledgeBase?.knowledge_base_name;
      return await invoke("kg_semantic_search", { kbName: name, query, topK, nodeTypes });
    },
    async kgSuggestRelations(kbName, nodeId, topK = 10, minScore = 0.6) {
      const name = kbName || this.selectedKnowledgeBase?.knowledge_base_name;
      return await invoke("kg_suggest_relations", { kbName: name, nodeId, topK, minScore });
    },
    async kgSuggestTags(kbName, nodeId, candidateTags = null, topK = 5, minScore = 0.55) {
      const name = kbName || this.selectedKnowledgeBase?.knowledge_base_name;
      return await invoke("kg_suggest_tags", { kbName: name, nodeId, candidateTags, topK, minScore });
    },
    async kgConfirmRelation(kbName, fromId, toId, relation = "related", strength = 0.5) {
      const name = kbName || this.selectedKnowledgeBase?.knowledge_base_name;
      return await invoke("kg_confirm_relation", { kbName: name, fromId, toId, relation, strength });
    },
    async kgAddTag(kbName, nodeId, tag, source = "manual", score = 1.0) {
      const name = kbName || this.selectedKnowledgeBase?.knowledge_base_name;
      return await invoke("kg_add_tag", { kbName: name, nodeId, tag, source, score });
    },
    async kgRemoveTag(kbName, nodeId, tag) {
      const name = kbName || this.selectedKnowledgeBase?.knowledge_base_name;
      return await invoke("kg_remove_tag", { kbName: name, nodeId, tag });
    },
    async kgRemoveRelation(kbName, fromId, toId, relation = null) {
      const name = kbName || this.selectedKnowledgeBase?.knowledge_base_name;
      return await invoke("kg_remove_relation", { kbName: name, fromId, toId, relation });
    },
    // 人工知识层(可移植快照)导出 / 导入 —— 供管理器查看与多机同步收尾使用。
    async kgExportManual(kbName) {
      const name = kbName || this.selectedKnowledgeBase?.knowledge_base_name;
      return await invoke("kg_export_manual", { kbName: name });
    },
    async kgImportManual(kbName, bundle = null) {
      const name = kbName || this.selectedKnowledgeBase?.knowledge_base_name;
      return await invoke("kg_import_manual", { kbName: name, bundle });
    },
    async kgWriteManualSnapshot(kbName) {
      const name = kbName || this.selectedKnowledgeBase?.knowledge_base_name;
      return await invoke("kg_write_manual_snapshot", { kbName: name });
    },
    // 机制B(行级精细同步)开关 —— 默认关闭,启用后知识库同步会额外双向按行合并人工层。
    async kgGetManualSync() {
      return await invoke("kg_get_manual_sync");
    },
    async kgSetManualSync(enabled) {
      return await invoke("kg_set_manual_sync", { enabled });
    },
    async kgEmbedKb(kbName) {
      const name = kbName || this.selectedKnowledgeBase?.knowledge_base_name;
      return await invoke("kg_embed_kb", { kbName: name });
    },
    async kgEnsureEmbeddingService() {
      return await invoke("kg_ensure_embedding_service");
    },
    async kgEmbeddingStatus() {
      return await invoke("kg_embedding_status");
    },
    async kgSetEmbeddingModel(modelKey) {
      return await invoke("kg_set_embedding_model", { modelKey });
    },
    async kgExtractConcepts(kbName, relPath = null, modelid = 6, maxFiles = 50) {
      const name = kbName || this.selectedKnowledgeBase?.knowledge_base_name;
      return await invoke("kg_extract_concepts", { kbName: name, relPath, modelid, maxFiles });
    },
    async kgSaveLayout(kbName, view, points) {
      const name = kbName || this.selectedKnowledgeBase?.knowledge_base_name;
      return await invoke("kg_save_layout", { kbName: name, view, points });
    },
    async kgLoadLayout(kbName, view) {
      const name = kbName || this.selectedKnowledgeBase?.knowledge_base_name;
      return await invoke("kg_load_layout", { kbName: name, view });
    },
    // 后端 Rust 力导向布局(relation 视角)。返回 [{node_id,x,y,z}],并已写入 kg_layout 缓存。
    async kgComputeLayout(kbName, view = "relation", force = false) {
      const name = kbName || this.selectedKnowledgeBase?.knowledge_base_name;
      return await invoke("kg_compute_layout", { kbName: name, view, force });
    },

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

    // ─── kg3d 通信总线 ───
    // kg3d 组件选中节点时调用,边栏据此渲染操作面板,并写入访问历史。
    setKg3dSelected(node) {
      this.kg3dSelected = node || null;
      if (node && node.id) {
        const entry = { id: node.id, name: node.name, type: node.type, ts: Date.now() };
        this.kg3dHistory = [entry, ...this.kg3dHistory.filter((x) => x.id !== node.id)].slice(0, 30);
      }
    },
    // 边栏 → kg3d 的动作请求(focus/open/suggestRelations/suggestTags/reload/flyTo...)
    requestKg3dAction(type, payload = null) {
      this.kg3dActionRequest = { type, payload, ts: Date.now() };
    },
    setKg3dRelations(rels) {
      this.kg3dRelations = Array.isArray(rels) ? rels : [];
    },
    setKg3dSuggestions(list) {
      this.kg3dSuggestions = Array.isArray(list) ? list : [];
    },
    toggleKg3dBookmark(node) {
      if (!node || !node.id) return;
      const exists = this.kg3dBookmarks.some((x) => x.id === node.id);
      if (exists) {
        this.kg3dBookmarks = this.kg3dBookmarks.filter((x) => x.id !== node.id);
      } else {
        this.kg3dBookmarks = [
          { id: node.id, name: node.name, type: node.type },
          ...this.kg3dBookmarks,
        ].slice(0, 50);
      }
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
