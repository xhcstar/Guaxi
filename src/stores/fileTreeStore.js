import { defineStore } from "pinia";
import { invoke } from "@tauri-apps/api/core";
import { createLogger } from "../utils/logger";

const logger = createLogger("fileTreeStore");
const console = {
  debug: (...args) => logger.debug(...args),
  log: (...args) => logger.info(...args),
  info: (...args) => logger.info(...args),
  warn: (...args) => logger.warn(...args),
  error: (...args) => logger.error(...args),
};

const classifyReadFileError = (error) => {
  const msg = String(error?.message || error || "");
  const lower = msg.toLowerCase();

  if (
    lower.includes("stream did not contain valid utf-8") ||
    lower.includes("invalid utf") ||
    lower.includes("utf-8") ||
    lower.includes("the data is invalid")
  ) {
    return "UNSUPPORTED";
  }

  if (
    lower.includes("os error 2") ||
    lower.includes("os error 3") ||
    lower.includes("not found") ||
    lower.includes("no such file") ||
    lower.includes("cannot find the file") ||
    lower.includes("系统找不到") ||
    lower.includes("文件不存在")
  ) {
    return "NOT_FOUND";
  }

  return "READ_FAILED";
};

const normalizeTabPath = (p) => String(p || "").replace(/\\/g, "/").toLowerCase();

export const useFileTreeStore = defineStore("fileTreeStore", {
  state: () => ({
    fileTree: [],
    selectedFile: null,
    repopathlist: [],
    activeRepoPath: null, // 当前激活的笔记库路径
    openTabs: [],
    activeTab: null,
    activeTabLeft: null,
    activeTabRight: null,
    layoutMode: 'single',
    splitIndex: null,
    activeRegion: 'left',
    // 新增：用于跟踪仓库展开状态的Map
    expandedRepos: new Map(), // key: repoPath, value: boolean
    currentRequestPath: null,
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

      const requestedPath = result?.path;
      const requestedKey = normalizeTabPath(requestedPath);
      const existingTab = this.openTabs.find((tab) => normalizeTabPath(tab.path) === requestedKey);

      // 获取文件内容（如果未加载）
      if (!existingTab) {
        const res = await this.fetchFileContent(requestedPath);
        if (res?.ok) {
          await this.openFileInTab(requestedPath, res.content, {
            unsupported: !!res.unsupported,
          });
        } else {
          throw new Error(res?.code || "READ_FAILED");
        }
      } else if (existingTab.path !== requestedPath) {
        console.debug('[fileTreeOpenSearchResult] Reusing tab via normalized path match', {
          requested: requestedPath,
          actual: existingTab.path
        })
      }

      const activePath = existingTab?.path || requestedPath;

      // 激活标签页并同步 selectedFile
      this.setActiveTab(activePath);
      const active = this.openTabs.find((tab) => normalizeTabPath(tab.path) === normalizeTabPath(activePath));

      // 返回需要高亮的信息
      return {
        path: active?.path || activePath,
        lineNumber: result.line_number,
        matches: result.matches,
        content: active ? active.content : this.selectedFile?.content || "",
        text: result.searchText, // 这里改为 text
      };
    },
    // 关闭标签页
    closeTab(path, region) {
      const index = this.openTabs.findIndex((tab) => tab.path === path);
      if (index === -1) return;

      const beforeSplitIndex = this.splitIndex;
      const beforeLayoutMode = this.layoutMode;
      const inferredRegion =
        beforeLayoutMode !== 'single' && beforeSplitIndex != null
          ? index < beforeSplitIndex
            ? 'left'
            : 'right'
          : 'left';

      const closingRegion = region || inferredRegion;

      this.openTabs.splice(index, 1);

      if (this.openTabs.length === 0) {
        this.activeTab = null;
        this.activeTabLeft = null;
        this.activeTabRight = null;
        this.selectedFile = null;
        this.layoutMode = 'single';
        this.splitIndex = null;
        this.activeRegion = 'left';
        return;
      }

      const leftCountAfter =
        this.layoutMode !== 'single' && beforeSplitIndex != null
          ? Math.max(0, Math.min(beforeSplitIndex, this.openTabs.length))
          : this.openTabs.length;

      const pickNextInRegion = (targetRegion) => {
        if (this.openTabs.length === 0) return null;
        if (this.layoutMode === 'single' || beforeSplitIndex == null) {
          const nextIndex = Math.min(index, this.openTabs.length - 1);
          return this.openTabs[nextIndex]?.path || null;
        }

        if (targetRegion === 'left') {
          const lastLeftIndex = Math.max(0, leftCountAfter - 1);
          const nextIndex = Math.min(index, lastLeftIndex);
          return this.openTabs[nextIndex]?.path || null;
        }

        const start = Math.max(0, leftCountAfter);
        if (start >= this.openTabs.length) return null;
        const nextIndex = Math.min(Math.max(index, start), this.openTabs.length - 1);
        return this.openTabs[nextIndex]?.path || this.openTabs[start]?.path || null;
      };

      if (closingRegion === 'left' && this.activeTabLeft === path) {
        this.activeTabLeft = pickNextInRegion('left');
      }
      if (closingRegion === 'right' && this.activeTabRight === path) {
        this.activeTabRight = pickNextInRegion('right');
      }

      if (this.activeTab === path) {
        const nextFocused =
          closingRegion === 'right'
            ? this.activeTabRight || this.activeTabLeft
            : this.activeTabLeft || this.activeTabRight;
        this.setActiveTab(nextFocused, nextFocused ? closingRegion : 'left');
      }

      if (this.layoutMode !== 'single' && this.splitIndex != null) {
        if (index < this.splitIndex) {
          this.splitIndex = Math.max(0, this.splitIndex - 1);
        }
        const leftCount = this.splitIndex ?? 0;
        const rightCount = this.openTabs.length - leftCount;
        if (leftCount === 0 || rightCount === 0) {
          this.layoutMode = 'single';
          this.splitIndex = null;
          this.activeRegion = 'left';
          this.activeTabRight = null;
          this.activeTabLeft = this.activeTab;
        }
      }
    },

    // 生成预览内容
    generatePreview(content) {
      return content.length > 50 ? content.substring(0, 50) + "..." : content;
    },
    // 获取文件树数据
    async fetchFileTree(path, options = {}) {
      const { force = false } = options || {};
      console.log("开始获取文件树数据，路径:", path);
      if (!path) {
        this.fileTree = [];
        return;
      }
      if (!force && this.currentRequestPath === path) return; // 防止重复请求

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
    async openFileInTab(path, content, options = {}) {
      const fileName = path.split(/[\\/]/).pop();

      // 检查是否已存在相同路径的标签页（兼容 Windows 下 \ 与 / 以及大小写差异）
      const desiredKey = normalizeTabPath(path);
      const existingTab = this.openTabs.find((tab) => normalizeTabPath(tab.path) === desiredKey);
      if (existingTab) {
        this.setActiveTab(existingTab.path, this.layoutMode === 'single' ? 'left' : this.activeRegion);
        return;
      }

      let fileContent = content;
      let unsupported = !!options.unsupported;

      // 如果未提供内容，则获取内容
      if (fileContent === undefined || fileContent === null) {
        const res = await this.fetchFileContent(path);
        if (!res?.ok) {
          console.error("获取文件内容失败，不创建标签页");
          return;
        }
        fileContent = res.content;
        unsupported = !!res.unsupported;
      }

      const newTab = {
        path,
        fileName,
        content: fileContent,
        savedContent: fileContent,
        dirty: false,
        preview: this.generatePreview(fileContent),
        unsupported,
      };

      if (this.layoutMode === 'single' || this.splitIndex == null) {
        this.openTabs.push(newTab);
      } else if (this.activeRegion === 'left') {
        const insertIndex = this.splitIndex;
        this.openTabs.splice(insertIndex, 0, newTab);
        this.splitIndex += 1;
      } else {
        this.openTabs.push(newTab);
      }

      this.setActiveTab(path);
    },

    // 获取文件内容
    async fetchFileContent(path) {
      try {
        const content = await invoke("read_file_content", { path });
        this.selectedFile = { path, content, unsupported: false };
        return { ok: true, content, unsupported: false };
      } catch (error) {
        const code = classifyReadFileError(error);
        const errorMessage = String(error?.message || error || "");

        if (code === "UNSUPPORTED") {
          const placeholder = `该文件为二进制文件或包含非 UTF-8 内容，当前不支持在编辑器中打开。\n\n路径：${path}\n\n你仍可通过右键菜单“在文件浏览器中打开”来查看该文件。`;
          this.selectedFile = {
            path,
            content: placeholder,
            unsupported: true,
            error: errorMessage,
          };
          return { ok: true, content: placeholder, unsupported: true };
        }

        console.error("获取文件内容失败:", error);
        return { ok: false, code, error: errorMessage };
      }
    },

    // 更新文件内容
    async updateFileContent(path, content) {
      try {
        await invoke("write_file_content", { path, content });

        const tabIndex = this.openTabs.findIndex((tab) => tab.path === path);
        if (tabIndex !== -1) {
          this.openTabs[tabIndex].content = content;
          this.openTabs[tabIndex].savedContent = content;
          this.openTabs[tabIndex].dirty = false;
          this.openTabs[tabIndex].preview = this.generatePreview(content);
        }

        if (this.selectedFile && this.selectedFile.path === path) {
          this.selectedFile.content = content;
        } else {
          console.warn("当前选中的文件与更新的文件不匹配，未更新 selectedFile");
        }
      } catch (error) {
        console.error("更新文件内容失败:", error);
      }
    },

    updateTabContent(path, content) {
      const tabIndex = this.openTabs.findIndex((tab) => tab.path === path);
      if (tabIndex !== -1) {
        const tab = this.openTabs[tabIndex];
        tab.content = content;
        tab.preview = this.generatePreview(content);
        tab.dirty = (tab.savedContent ?? "") !== content;

        if (this.selectedFile && this.selectedFile.path === path) {
          this.selectedFile.content = content;
        }
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

    // 新增：统一设置激活标签并同步 selectedFile
    setActiveTab(path, region) {
      const desiredKey = normalizeTabPath(path);
      const resolved = path
        ? this.openTabs.find((t) => normalizeTabPath(t.path) === desiredKey)
        : null;
      if (resolved) {
        path = resolved.path;
      }

      this.activeTab = path;

      if (!path) {
        this.selectedFile = null;
        if (this.layoutMode === 'single') {
          this.activeTabLeft = null;
          this.activeTabRight = null;
        }
        return;
      }

      const index = this.openTabs.findIndex((t) => normalizeTabPath(t.path) === normalizeTabPath(path));
      const tab = index !== -1 ? this.openTabs[index] : null;

      if (tab) {
        this.selectedFile = {
          path: tab.path,
          content: tab.content,
          unsupported: !!tab.unsupported,
        };
      }

      let targetRegion = region || this.activeRegion || 'left';
      if (this.layoutMode !== 'single' && this.splitIndex != null && index !== -1) {
        targetRegion = index < this.splitIndex ? 'left' : 'right';
      }

      this.activeRegion = targetRegion;

      if (this.layoutMode === 'single') {
        this.activeTabLeft = path;
        this.activeTabRight = null;
        return;
      }

      if (targetRegion === 'left') {
        this.activeTabLeft = path;
        if (!this.activeTabRight && this.splitIndex != null) {
          this.activeTabRight = this.openTabs[this.splitIndex]?.path || null;
        }
      } else {
        this.activeTabRight = path;
        if (!this.activeTabLeft && this.splitIndex != null) {
          this.activeTabLeft = this.openTabs[Math.max(0, this.splitIndex - 1)]?.path || this.openTabs[0]?.path || null;
        }
      }
    },
    setLayoutMode(mode) {
      if (mode === this.layoutMode) return;
      this.layoutMode = mode;
      if (mode === 'single') {
        this.splitIndex = null;
        this.activeRegion = 'left';
        this.activeTabLeft = this.activeTab;
        this.activeTabRight = null;
      } else if (this.splitIndex == null) {
        const idx = this.activeTab
          ? this.openTabs.findIndex((t) => t.path === this.activeTab)
          : -1;
        this.splitIndex = idx > 0 ? idx : 0;

        this.activeTabLeft = this.openTabs[Math.max(0, this.splitIndex - 1)]?.path || this.openTabs[0]?.path || null;
        this.activeTabRight = this.openTabs[this.splitIndex]?.path || null;

        if (this.activeTab) {
          this.setActiveTab(this.activeTab);
        }
      }
    },

    setActiveRegion(region) {
      this.activeRegion = region;
    },

    splitCurrentTab(mode) {
      if (!this.activeTab || this.openTabs.length === 0) return;
      const idx = this.openTabs.findIndex((t) => t.path === this.activeTab);
      if (idx === -1) return;

      if (this.layoutMode === 'single') {
        const [tab] = this.openTabs.splice(idx, 1);
        this.openTabs.push(tab);

        this.layoutMode = mode;
        this.splitIndex = Math.max(0, this.openTabs.length - 1);

        this.activeTabRight = tab.path;
        this.activeTabLeft = this.openTabs[Math.max(0, this.splitIndex - 1)]?.path || this.openTabs[0]?.path || null;

        this.setActiveTab(tab.path, 'right');
        return;
      }

      this.layoutMode = mode;
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
