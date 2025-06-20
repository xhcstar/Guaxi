// stores/knowledgeBaseStore.js
import { defineStore } from "pinia";
import { invoke } from "@tauri-apps/api/core";

export const useKnowledgeBaseStore = defineStore("knowledgeBase", {
  state: () => ({
    knowledgeBases: [],
    loading: false,
    showForm: false, // 控制表单显示
    formData: {
      knowledgeBaseName: "",
      knowledgeBaseTags: "",
      knowledgeBaseDesc: "",
    },
  }),

  actions: {
    // 获取知识库列表
    async fetchKnowledgeBases() {
      this.loading = true;
      try {
        this.knowledgeBases = await invoke("get_knowledge_base_list");
      } finally {
        this.loading = false;
      }
    },

    // 创建知识库
    async createKnowledgeBase() {
      if (!this.formData.knowledgeBaseName) {
        throw new Error("知识库名字不能为空");
      }

      try {
        await invoke("create_knowledge_base", {
          input: {
            knowledge_base_name: this.formData.knowledgeBaseName,
            knowledge_base_tags: this.formData.knowledgeBaseTags,
            knowledge_base_description: this.formData.knowledgeBaseDesc,
          }
        });

        this.resetForm();
        await this.fetchKnowledgeBases();
      } catch (e) {
        console.error("创建知识库失败:", e);
        throw e;
      }
    },

    // 重置表单
    resetForm() {
      this.formData = {
        knowledgeBaseName: "",
        knowledgeBaseTags: "",
        knowledgeBaseDesc: "",
      };
    },

    // 切换表单显示
    toggleForm(show) {
      this.showForm = show !== undefined ? show : !this.showForm;
    },
  },
});
