import { defineStore } from "pinia";
import { invoke } from "@tauri-apps/api/core";

export const useFileTreeStore = defineStore("fileTreeStore", {
  state: () => ({
    fileTree: [],
    selectedFile: null,
    repopathlist: [],
    activeRepoPath: null, // 当前激活的笔记库路径
    openTabs: [],
    activeTab: null,
    // 新增：用于跟踪仓库展开状态的Map
    expandedRepos: new Map(), // key: repoPath, value: boolean
  }),
  actions: {
    // 新增：切换仓库展开状态
    toggleRepoExpand(path) {
      this.expandedRepos.set(path, !this.expandedRepos.get(path));
    },

    // 新增：检查仓库是否展开
    isRepoExpanded(path) {
      return this.expandedRepos.get(path) || false;
    },
    // 新增设置仓库列表方法
    setRepopathlist(paths) {
      this.repopathlist = paths;
      console.log("设置仓库路径列表:", paths);
    },

    // 新增设置激活路径方法
    setActiveRepoPath(path) {
      this.activeRepoPath = path;
      console.log("设置激活仓库路径:", path);
    },
    // 新增：专门处理搜索结果的打开
    async fileTreeOpenSearchResult(result) {
      console.log("打开搜索结果:", result);
      // 获取文件内容（如果未加载）
      if (!this.openTabs.some((tab) => tab.path === result.path)) {
        const content = await this.fetchFileContent(result.path);
        this.openFileInTab(result.path, content);
      }

      // 激活标签页
      this.activeTab = result.path;
      this.selectedFile = {
        path: result.path,
        content: this.openTabs.find((tab) => tab.path === result.path).content,
      };

      // console.log("this.selectedFile.content:", this.selectedFile.content);
      // 返回需要高亮的信息
      return {
        path: result.path,
        lineNumber: result.line_number,
        matches: result.matches,
        content: this.selectedFile.content,
        text: result.searchText, // 这里改为 text
      };
    },
    // 关闭标签页
    closeTab(path) {
      this.openTabs = this.openTabs.filter((tab) => tab.path !== path);
      if (this.activeTab === path) {
        this.activeTab =
          this.openTabs.length > 0 ? this.openTabs[0].path : null;
      }
    },

    // 生成预览内容
    generatePreview(content) {
      return content.length > 50 ? content.substring(0, 50) + "..." : content;
    },
    // 获取文件树数据
    async fetchFileTree(path) {
      console.log("开始获取文件树数据，路径:", path);
      if (this.currentRequestPath === path) return; // 防止重复请求

      this.currentRequestPath = path;
      try {
        const result = await invoke("get_file_tree", { path });
        console.log("获取文件树数据成功:", result);
        this.fileTree = result;
      } catch (error) {
        console.error("获取文件树数据失败:", error);
      } finally {
        this.currentRequestPath = null;
      }
    },

    // 修改 openFileInTab 方法，防止重复打开
    async openFileInTab(path, content) {
      const fileName = path.split(/[\\/]/).pop();

      // 检查是否已存在相同路径的标签页
      const existingTab = this.openTabs.find((tab) => tab.path === path);
      if (existingTab) {
        this.activeTab = path;
        return;
      }

      // 如果未提供内容，则获取内容
      const fileContent = content || (await this.fetchFileContent(path));
      if (fileContent === null) {
        console.error("获取文件内容失败，不创建标签页");
        return;
      }

      this.openTabs.push({
        path,
        fileName,
        content: fileContent,
        preview: this.generatePreview(fileContent),
      });

      this.activeTab = path;
      this.selectedFile = { path, content: fileContent };
    },

    // 获取文件内容
    async fetchFileContent(path) {
      //   console.log('开始获取文件内容，路径:', path)
      try {
        const content = await invoke("read_file_content", { path });
        // console.log('获取文件内容成功:', content)
        this.selectedFile = { path, content };
        return content;
      } catch (error) {
        console.error("获取文件内容失败:", error);
        return null;
      }
    },

    // 更新文件内容
    async updateFileContent(path, content) {
      //   console.log('开始更新文件内容，路径:', path)
      try {
        await invoke("write_file_content", { path, content });
        // console.log('更新文件内容成功')
        if (this.selectedFile && this.selectedFile.path === path) {
          this.selectedFile.content = content;
          //   console.log('更新 selectedFile 内容成功')
        } else {
          console.warn("当前选中的文件与更新的文件不匹配，未更新 selectedFile");
        }
      } catch (error) {
        console.error("更新文件内容失败:", error);
      }
    },

    // 设置当前激活的笔记库路径
    setActiveRepoPath(path) {
      this.activeRepoPath = path;
    },

    // 更新笔记库路径列表
    setRepopathlist(paths) {
      this.repopathlist = paths;
    },
    // 更新 repopathlist 并保存到配置文件
    async updateRepopathlist(paths) {
      console.log("更新笔记库路径列表:", paths);
      try {
        await invoke("set_repopathlist", { paths }); // 确保传递的参数是一个数组
        this.repopathlist = paths;
      } catch (error) {
        console.error("更新笔记库路径列表失败:", error);
      }
    },

    // 添加新的笔记库路径
    async addRepoPath(path) {
      if (!this.repopathlist.includes(path)) {
        const newPaths = [...this.repopathlist, path];
        await this.updateRepopathlist(newPaths);
      }
    },

    // 移除笔记库路径
    async removeRepoPath(path) {
      const newPaths = this.repopathlist.filter((p) => p !== path);
      await this.updateRepopathlist(newPaths);
      if (this.activeRepoPath === path) {
        this.activeRepoPath = null;
      }
    },
  },
});
