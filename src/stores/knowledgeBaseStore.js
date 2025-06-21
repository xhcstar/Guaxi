import { defineStore } from "pinia";
import { invoke } from "@tauri-apps/api/core";

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
  }),

  actions: {
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
        return;
      }

      // 深拷贝并标准化路径
      this.selectedKnowledgeBase = {
        ...kb,
        location: kb.location.replace(/\\/g, "/"),
      };
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
  },
});
