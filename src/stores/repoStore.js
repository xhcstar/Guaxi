import { defineStore } from "pinia";
import { invoke } from "@tauri-apps/api/core";

export const repoStore = defineStore("repoStore", {
  state: () => ({
    files: [], // 存储文件数据
    notes: [], // 存储笔记数据
    repos: [], // 存储知识库列表
    currentDatabase: "", // 默认选中的数据库
    librariesRoot: "",
  }),
  actions: {
    async fetchRepos() {
      try {
        const repos = await invoke("get_knowledge_repos");

        // 按照字母表顺序排序（不区分大小写）
        this.repos = repos.sort((a, b) => {
          const nameA = a.toLowerCase();
          const nameB = b.toLowerCase();
          if (nameA < nameB) return -1;
          if (nameA > nameB) return 1;
          return 0;
        });

        if (!this.currentDatabase && this.repos.length > 0) {
          this.currentDatabase = this.repos[0];
        }

        console.log("排序后的知识库列表:", this.repos);
      } catch (error) {
        console.error("获取知识库失败:", error);
        throw error;
      }
    },

    async fetchFiles(key) {
      try {
        // 调用获取文件数据的接口
        const files = await invoke("get_imported_files", {
          knowledgeRepo: key,
        });
        console.log("get_imported_files", files);

        // 数据转换：将后端文件数据映射为前端需要的数据结构
        this.files = files.map((file) => ({
          id: file.id,
          importFileName: file.import_file_name,
          description: file.description,
          importedFileTags: file.tags.join(","), // 将 tags 数组转换为字符串
          type: file.file_type,
          library: file.library,
          importFileCreateTime: file.import_file_create_time,
          importFileSize: file.import_file_size,
          convetToHtmlFilename: "",
          knowledgeRepo: file.knowledge_repo,
        }));
        console.log("文件数据获取成功", this.files);
      } catch (error) {
        console.error("Error fetching data:", error);
        this.notes = []; // 重置笔记数据
        this.files = []; // 重置文件数据
      }
    },

    // 添加导入的文件
    addImportedFile(file) {
      this.files.push(file);
    },

    // 添加导入的笔记
    addImportedNote(note) {
      this.notes.push(note);
    },

    set_librariesRoot(path) {
      this.librariesRoot = path;
      console.log("set_librariesRoot", this.librariesRoot);
    },

    async get_librariesRoot() {
      try {
        this.librariesRoot = await invoke("get_libraries_root");
        console.log("初始化：获取当librariesRoot路径成功", this.librariesRoot);
      } catch (error) {
        console.error("获取当librariesRoot仓库路径失败", error);
      }
    },
  },
});
