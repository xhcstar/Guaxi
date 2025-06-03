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
  }),
  actions: {
    // 在标签页中打开文件
    openFileInTab(path, content) {
      // 从路径中提取文件名
      const fileName = path.split(/[\\/]/).pop();
      console.log("文件名:", fileName);
      if (!this.openTabs.some((tab) => tab.path === path)) {
        this.openTabs.push({
          path,
          fileName,  // 添加文件名字段
          content,
          preview: this.generatePreview(content),
        });
      }
      this.activeTab = path;
      this.selectedFile = { path, content };
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
      try {
        const result = await invoke("get_file_tree", { path });
        console.log("获取文件树数据成功:", result);
        this.fileTree = result;
      } catch (error) {
        console.error("获取文件树数据失败:", error);
      }
    },

    // 获取文件内容
    async fetchFileContent(path) {
      //   console.log('开始获取文件内容，路径:', path)
      try {
        const content = await invoke("read_file_content", { path });
        // console.log('获取文件内容成功:', content)
        this.selectedFile = { path, content };
        return content
      } catch (error) {
        console.error("获取文件内容失败:", error);
        return null
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
