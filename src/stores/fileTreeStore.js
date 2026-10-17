import { defineStore } from "pinia";
import { invoke } from "@tauri-apps/api/core";
import { createLogger } from "../utils/logger";
import { useEditorBridgeStore } from "./editorBridgeStore";

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

// 设计说明：文件树允许创建大小写不同的同名文件，tab key 也必须保留大小写，
// 否则 README.md 与 readme.md 会在编辑器层被误判为同一个文件。
const normalizeTabPath = (p) => String(p || "").replace(/\\/g, "/");
const normalizeNavigationRootKey = (p) => normalizeTabPath(p).replace(/\/+$/, "");
const normalizeEditorContent = (content) => String(content ?? "").replace(/\r\n?/g, "\n");
const EVENT_CELLMETA_FILE_CLOSED = "guaxi_cellmeta_file_closed";
// 在途写盘的 promise 集合：路径变更前会等待它们落地，防止旧路径的写入在移动/删除后才完成。
const inflightContentWrites = new Set();

export const useFileTreeStore = defineStore("fileTreeStore", {
  state: () => ({
    fileTree: [],
    selectedFile: null,
    repopathlist: [],
    activeRepoPath: null, // 当前激活的笔记库路径
    // 设计说明：openTabs 退化为「按路径唯一的文件登记表」(持有 content/savedContent/dirty/preview 等)，
    // 同一文件在两侧分屏时共享同一条登记。每个 pane 的标签顺序由独立的路径列表维护，允许同一路径
    // 同时出现在两侧 → 实现同文件分屏。取代旧的「openTabs 数组 + splitIndex 切分点」模型。
    openTabs: [],
    activeTab: null,
    activeTabLeft: null,
    activeTabRight: null,
    layoutMode: 'single',
    leftTabPaths: [],
    rightTabPaths: [],
    activeRegion: 'left',
    // 重命名/移动/删除进行中：暂停所有写盘，避免异步写回把旧路径重新创建出来。
    pathMutationDepth: 0,
    // 左侧边栏当前浏览上下文：
    // 1. 知识库、普通文件、任务三种模式使用不同的根目录与树数据，不能只依赖 activeRepoPath。
    // 2. 编辑器面包屑读取这份共享状态，确保左栏模式/目录/任务分类切换后默认根路径立即同步。
    sidebarNavigationMode: "kb",
    sidebarNavigationRootPath: null,
    sidebarNavigationTree: [],
    sidebarNavigationVersion: 0,
    // 按根目录保存各模式的树快照。切换模式后旧根仍保留，已打开标签切回时可按文件实际路径恢复正确候选。
    sidebarNavigationContexts: {},
    // 新增：用于跟踪仓库展开状态的Map
    expandedRepos: new Map(), // key: repoPath, value: boolean
    currentRequestPath: null,
  }),
  actions: {
    setSidebarNavigationContext({ mode = "kb", rootPath = null, tree = [] } = {}) {
      const nextMode = ["kb", "plain", "task"].includes(mode) ? mode : "kb";
      const nextRootPath = rootPath || null;
      const nextTree = Array.isArray(tree) ? tree : [];

      this.sidebarNavigationMode = nextMode;
      this.sidebarNavigationRootPath = nextRootPath;
      this.sidebarNavigationTree = nextTree;
      // 树节点可能通过懒加载原地追加 children，版本号用于通知面包屑重建路径索引。
      this.sidebarNavigationVersion += 1;

      const rootKey = normalizeNavigationRootKey(nextRootPath);
      if (rootKey) {
        this.sidebarNavigationContexts = {
          ...this.sidebarNavigationContexts,
          [rootKey]: {
            mode: nextMode,
            rootPath: nextRootPath,
            tree: nextTree,
            version: this.sidebarNavigationVersion,
          },
        };
      }

      console.info("[SidebarNavigation] context updated", {
        mode: nextMode,
        rootPath: nextRootPath,
        nodeCount: nextTree.length,
        version: this.sidebarNavigationVersion,
      });
    },

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
    // ───── 分屏 pane 标签列表辅助 ─────
    otherRegion(region) {
      return region === 'right' ? 'left' : 'right';
    },
    paneListFor(region) {
      return region === 'right' ? this.rightTabPaths : this.leftTabPaths;
    },
    paneHasPath(region, path) {
      const key = normalizeTabPath(path);
      return this.paneListFor(region).some((p) => normalizeTabPath(p) === key);
    },
    addPathToPane(region, path) {
      if (!path || this.paneHasPath(region, path)) return;
      if (region === 'right') this.rightTabPaths.push(path);
      else this.leftTabPaths.push(path);
    },
    removePathFromPane(region, path) {
      const key = normalizeTabPath(path);
      const next = this.paneListFor(region).filter((p) => normalizeTabPath(p) !== key);
      if (region === 'right') this.rightTabPaths = next;
      else this.leftTabPaths = next;
    },
    pathOpenAnywhere(path) {
      const key = normalizeTabPath(path);
      return this.leftTabPaths.some((p) => normalizeTabPath(p) === key)
        || this.rightTabPaths.some((p) => normalizeTabPath(p) === key);
    },
    // 当某路径不再被任何 pane 引用时，从登记表移除其内容条目。
    pruneOrphanTab(path) {
      if (this.pathOpenAnywhere(path)) return;
      const key = normalizeTabPath(path);
      this.openTabs = this.openTabs.filter((t) => normalizeTabPath(t.path) !== key);
    },
    // 任意标签/朝向变化后，归一化分屏布局（右空回单屏、左空则右转左、全空则重置）。
    normalizeSplitLayout() {
      if (this.leftTabPaths.length === 0 && this.rightTabPaths.length === 0) {
        this.activeTab = null;
        this.activeTabLeft = null;
        this.activeTabRight = null;
        this.selectedFile = null;
        this.layoutMode = 'single';
        this.activeRegion = 'left';
        return;
      }
      if (this.layoutMode === 'single') {
        this.rightTabPaths = [];
        this.activeTabRight = null;
        return;
      }
      if (this.rightTabPaths.length === 0) {
        this.layoutMode = 'single';
        this.activeRegion = 'left';
        this.activeTabRight = null;
        return;
      }
      if (this.leftTabPaths.length === 0) {
        // 左侧空：把右侧提升为左侧（保持 left 为主），回单屏。
        this.leftTabPaths = this.rightTabPaths;
        this.rightTabPaths = [];
        this.activeTabLeft = this.activeTabRight;
        this.activeTabRight = null;
        this.layoutMode = 'single';
        this.activeRegion = 'left';
      }
    },

    // 关闭标签页（按 pane 维度：仅从指定/激活侧移除；该路径不在任何侧时才删除其内容条目）
    closeTab(path, region) {
      const key = normalizeTabPath(path);
      const closingTab = this.openTabs.find((tab) => normalizeTabPath(tab.path) === key);
      if (!closingTab && !this.pathOpenAnywhere(path)) return;

      let closingRegion = region || this.activeRegion || 'left';
      if (this.layoutMode === 'single') closingRegion = 'left';
      else if (!this.paneHasPath(closingRegion, path)) {
        closingRegion = this.paneHasPath('right', path) ? 'right' : 'left';
      }

      // 记录该侧关闭前位置，用于回填相邻标签为激活。
      const listBefore = this.paneListFor(closingRegion);
      const closingIndex = listBefore.findIndex((p) => normalizeTabPath(p) === key);

      // 仅从该侧移除。
      this.removePathFromPane(closingRegion, path);

      // 回填该侧激活标签（若关闭的正是该侧激活标签）。
      const activeField = closingRegion === 'right' ? 'activeTabRight' : 'activeTabLeft';
      if (normalizeTabPath(this[activeField]) === key) {
        const listAfter = this.paneListFor(closingRegion);
        const nextIndex = Math.min(Math.max(0, closingIndex), listAfter.length - 1);
        this[activeField] = listAfter[nextIndex] || null;
      }

      // 该路径已不在任何 pane → 真正关闭文件：删除内容条目并发关闭事件。
      const trulyClosed = !this.pathOpenAnywhere(path);
      if (trulyClosed) this.pruneOrphanTab(path);

      // 归一化布局，再同步当前激活文件。
      this.normalizeSplitLayout();
      const focusRegion = this.activeRegion || 'left';
      const focusPath = (focusRegion === 'right' ? this.activeTabRight : this.activeTabLeft)
        || this.activeTabLeft || this.activeTabRight || null;
      this.setActiveTab(focusPath, focusPath ? focusRegion : 'left');

      if (trulyClosed) {
        const payload = {
          path: closingTab?.path || path,
          fileName: closingTab?.fileName || "",
          dirty: !!closingTab?.dirty,
          region: closingRegion,
        };
        // 设计说明：
        // 1. `.cellmeta` 自动提交属于后台整理动作，不能拖慢标签关闭和编辑器状态切换。
        // 2. setTimeout 把通知放到当前 UI 更新之后；提交失败由监听方记录日志，不打断关闭。
        // 3. 只在文件“真正关闭”(不再被任一分屏引用)时触发，避免同文件分屏关一侧就误提交。
        setTimeout(() => {
          try {
            window.dispatchEvent(new CustomEvent(EVENT_CELLMETA_FILE_CLOSED, { detail: payload }));
            console.info("[CellmetaAutoCommit] queued async commit after tab close", payload);
          } catch (error) {
            console.warn("[CellmetaAutoCommit] failed to queue tab-close commit event", {
              ...payload,
              error: String(error?.message || error || ""),
            });
          }
        }, 0);
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

    // 检查是否已存在相同路径的标签页：只统一路径分隔符，保留大小写差异。
    // 设计说明：文件树允许 README.md 与 readme.md 同时存在时，编辑器标签页也必须把它们视为两个文件。
      const desiredKey = normalizeTabPath(path);
      const existingTab = this.openTabs.find((tab) => normalizeTabPath(tab.path) === desiredKey);

      // options.region：显式指定落到哪一侧 pane（"在一侧打开"）。
      // 指定 right 且当前是单屏时先进入左右分屏——只把目标文件放到右侧，
      // 不像 splitCurrentTab 那样把当前文件复制过去，这才符合 VSCode 的 "Open to the Side"。
      const forcedRegion = options.region === 'right' || options.region === 'left'
        ? options.region
        : null;
      if (forcedRegion === 'right' && this.layoutMode === 'single') {
        this.layoutMode = 'vertical';
        if (!this.activeTabLeft) this.activeTabLeft = this.leftTabPaths[0] || null;
      }

      // 目标 pane：显式指定优先；否则单屏恒为 left、分屏时进入当前激活侧（"新文件去激活侧"）。
      const targetRegion = forcedRegion
        || (this.layoutMode === 'single' ? 'left' : (this.activeRegion || 'left'));
      if (existingTab) {
        // 已打开：聚焦其所在 pane（优先目标侧；否则它实际所在的另一侧），不重复创建标签。
        let focusRegion = targetRegion;
        if (forcedRegion) {
          // 显式"在一侧打开"：即使它已经在另一侧开着，也要在指定侧再挂一个标签
          // （同文件分屏是这个 store 明确支持的模型）。
          this.addPathToPane(forcedRegion, existingTab.path);
          focusRegion = forcedRegion;
        } else if (this.layoutMode === 'single') {
          this.addPathToPane('left', existingTab.path);
          focusRegion = 'left';
        } else if (this.paneHasPath(targetRegion, existingTab.path)) {
          focusRegion = targetRegion;
        } else if (this.paneHasPath(this.otherRegion(targetRegion), existingTab.path)) {
          focusRegion = this.otherRegion(targetRegion);
        } else {
          this.addPathToPane(targetRegion, existingTab.path);
          focusRegion = targetRegion;
        }
        this.setActiveTab(existingTab.path, focusRegion);
        try {
          const editorBridgeStore = useEditorBridgeStore();
          await editorBridgeStore.trackFile(existingTab.path, this);
        } catch (error) {
          console.warn("[EditorBridge] track existing tab failed", {
            path: existingTab.path,
            error: String(error?.message || error || ""),
          });
        }
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

      const normalizedContent = normalizeEditorContent(fileContent);

      const newTab = {
        path,
        fileName,
        content: normalizedContent,
        savedContent: normalizedContent,
        dirty: false,
        preview: this.generatePreview(normalizedContent),
        unsupported,
      };

      // 登记内容条目（按路径唯一），并把路径加入目标 pane 的标签列表。
      this.openTabs.push(newTab);
      this.addPathToPane(targetRegion, path);

      this.setActiveTab(path, targetRegion);
      try {
        const editorBridgeStore = useEditorBridgeStore();
        await editorBridgeStore.trackFile(path, this);
      } catch (error) {
        console.warn("[EditorBridge] track file after open failed", {
          path,
          error: String(error?.message || error || ""),
        });
      }
    },

    // 获取文件内容
    async fetchFileContent(path) {
      try {
        const rawContent = await invoke("read_file_content", { path });
        const content = normalizeEditorContent(rawContent);
        // 设计说明：
        // 1. fetchFileContent 只负责“读取”，不负责切换当前活动文件。
        // 2. 之前这里会提前写 selectedFile，导致 activeTab 仍指向旧文件时，
        //    编辑器却开始用“新文件内容”重绑旧文件 model，最终把最后打开的内容串到其他 tab。
        // 3. 当前活动文件的切换统一交给 openFileInTab / setActiveTab，避免读取阶段产生副作用。
        console.debug("[fileTreeStore] fetchFileContent success", {
          path,
          length: String(content || "").length,
        });
        return { ok: true, content, unsupported: false };
      } catch (error) {
        const code = classifyReadFileError(error);
        const errorMessage = String(error?.message || error || "");

        if (code === "UNSUPPORTED") {
          const placeholder = `该文件为二进制文件或包含非 UTF-8 内容，当前不支持在编辑器中打开。\n\n路径：${path}\n\n你仍可通过右键菜单“在文件浏览器中打开”来查看该文件。`;
          console.warn("[fileTreeStore] fetchFileContent unsupported", {
            path,
            error: errorMessage,
          });
          return { ok: true, content: placeholder, unsupported: true };
        }

        console.error("获取文件内容失败:", error);
        return { ok: false, code, error: errorMessage };
      }
    },

    // 更新文件内容
    // 设计说明：
    // 1. 预览卡片缓存需要感知文件是否已经变化，单纯使用路径作为 key 会在跨文件切换或文件更新后复用旧元数据。
    // 2. 这里统一向后端获取 mtime / size，供预览层拼装更稳定的缓存版本戳。
    // 3. 补充日志后，后续若仍有卡片串用问题，可以直接从 store 层确认 meta 是否过期。
    async fetchFileMeta(path) {
      try {
        const meta = await invoke("get_file_meta", { path });
        console.debug("[fileTreeStore] fetchFileMeta success", {
          path,
          mtimeMs: meta?.mtime_ms ?? null,
          fileSize: meta?.file_size ?? null,
        });
        return {
          ok: true,
          path: String(meta?.path || path || ""),
          mtimeMs: meta?.mtime_ms ?? null,
          fileSize: Number(meta?.file_size || 0),
          isDirectory: !!meta?.is_directory,
        };
      } catch (error) {
        const errorMessage = String(error?.message || error || "");
        console.warn("[fileTreeStore] fetchFileMeta failed", { path, error: errorMessage });
        return { ok: false, path: String(path || ""), error: errorMessage };
      }
    },

    async updateFileContent(path, content) {
      // 设计说明（防止“文件复活”）：
      // 1. 重命名/删除期间暂停写盘：in-flight 的自动保存若在 move/delete 之后落地，
      //    会在旧路径重新创建文件，或把内容写到已经不属于它的路径。
      // 2. 目标路径必须仍是打开中的标签：标签被关闭/删除/改名后，旧路径的延迟写盘直接丢弃。
      if (this.pathMutationDepth > 0) {
        console.warn("[fileTreeStore] write skipped during path mutation", { path });
        return false;
      }
      const writeKey = normalizeTabPath(path);
      const tracked = this.openTabs.some((tab) => normalizeTabPath(tab.path) === writeKey);
      if (!tracked) {
        console.warn("[fileTreeStore] write skipped for untracked path", { path });
        return false;
      }

      // 记录在途写入，路径变更（beginPathMutation）会等待它落地：
      // 否则 rename/delete 之后才完成的那一笔写盘会把旧路径重新创建出来。
      const writeTask = (async () => {
        try {
          const editorBridgeStore = useEditorBridgeStore();
          await editorBridgeStore.prepareWrite(path, "monaco", this);

          const normalizedContent = normalizeEditorContent(content);
          await invoke("write_file_content", { path, content: normalizedContent });

          const tabIndex = this.openTabs.findIndex((tab) => normalizeTabPath(tab.path) === writeKey);
          if (tabIndex !== -1) {
            this.openTabs[tabIndex].content = normalizedContent;
            this.openTabs[tabIndex].savedContent = normalizedContent;
            this.openTabs[tabIndex].dirty = false;
            this.openTabs[tabIndex].preview = this.generatePreview(normalizedContent);
          }

          if (this.selectedFile && normalizeTabPath(this.selectedFile.path) === writeKey) {
            this.selectedFile.content = normalizedContent;
          } else {
            console.warn("当前选中的文件与更新的文件不匹配，未更新 selectedFile", { path });
          }

          await editorBridgeStore.markAfterWrite(path, this);
          return true;
        } catch (error) {
          console.error("更新文件内容失败:", error);
          throw error;
        }
      })();

      inflightContentWrites.add(writeTask);
      try {
        return await writeTask;
      } finally {
        inflightContentWrites.delete(writeTask);
      }
    },

    // 进入路径变更：先暂停新写入，再等待已在途的写盘完成，避免它们落到旧路径上。
    async beginPathMutation() {
      this.pathMutationDepth += 1;
      if (inflightContentWrites.size) {
        await Promise.allSettled(Array.from(inflightContentWrites));
      }
    },

    endPathMutation() {
      this.pathMutationDepth = Math.max(0, this.pathMutationDepth - 1);
    },

    /**
     * 删除/重命名目录后，关闭其下所有已打开标签。
     *
     * 不关闭的话，这些标签会继续参与自动保存，把已经删掉的文件重新写回磁盘；
     * 或者把旧目录的内容写进新路径。关闭后由调用方决定激活哪个相邻标签。
     */
    dropTabsUnderPath(path) {
      const root = normalizeTabPath(path);
      if (!root) return;
      const isUnder = (candidate) => {
        const key = normalizeTabPath(candidate);
        return !!key && (key === root || key.startsWith(`${root}/`));
      };

      this.openTabs = this.openTabs.filter((tab) => !isUnder(tab?.path));
      this.leftTabPaths = this.leftTabPaths.filter((p) => !isUnder(p));
      this.rightTabPaths = this.rightTabPaths.filter((p) => !isUnder(p));
      if (isUnder(this.activeTabLeft)) this.activeTabLeft = this.leftTabPaths[0] || null;
      if (isUnder(this.activeTabRight)) this.activeTabRight = this.rightTabPaths[0] || null;

      this.normalizeSplitLayout();
      const focusPath = this.activeTabLeft || this.activeTabRight || null;
      this.setActiveTab(focusPath, focusPath ? (this.activeRegion || 'left') : 'left');
    },

    updateTabContent(path, content) {
      const pathKey = normalizeTabPath(path);
      const tabIndex = this.openTabs.findIndex((tab) => normalizeTabPath(tab.path) === pathKey);
      if (tabIndex !== -1) {
        const tab = this.openTabs[tabIndex];
        const normalizedContent = normalizeEditorContent(content);
        tab.content = normalizedContent;
        tab.preview = this.generatePreview(normalizedContent);
        tab.dirty = normalizeEditorContent(tab.savedContent) !== normalizedContent;

        if (this.selectedFile && normalizeTabPath(this.selectedFile.path) === pathKey) {
          this.selectedFile.content = normalizedContent;
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

    // 拖拽调整某一侧标签的顺序。toPath 为空表示拖到该侧末尾。
    reorderPaneTabs(region, fromPath, toPath, placeAfter = false) {
      const targetRegion = region === 'right' ? 'right' : 'left';
      const fromKey = normalizeTabPath(fromPath);
      const list = [...this.paneListFor(targetRegion)];
      const fromIndex = list.findIndex((p) => normalizeTabPath(p) === fromKey);
      if (fromIndex === -1) return;

      const [moved] = list.splice(fromIndex, 1);
      if (!toPath) {
        list.push(moved);
      } else {
        const toKey = normalizeTabPath(toPath);
        const toIndex = list.findIndex((p) => normalizeTabPath(p) === toKey);
        if (toIndex === -1) {
          list.push(moved);
        } else {
          list.splice(placeAfter ? toIndex + 1 : toIndex, 0, moved);
        }
      }

      if (targetRegion === 'right') this.rightTabPaths = list;
      else this.leftTabPaths = list;
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
        this.activeTabLeft = null;
        this.activeTabRight = null;
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

      if (this.layoutMode === 'single') {
        this.activeRegion = 'left';
        this.activeTabLeft = tab ? tab.path : path;
        this.activeTabRight = null;
        return;
      }

      // 分屏：默认进入指定/激活侧；若该侧并不包含此路径而另一侧包含，则纠正到它实际所在侧。
      let targetRegion = region || this.activeRegion || 'left';
      if (!this.paneHasPath(targetRegion, path) && this.paneHasPath(this.otherRegion(targetRegion), path)) {
        targetRegion = this.otherRegion(targetRegion);
      }
      this.activeRegion = targetRegion;
      if (targetRegion === 'right') this.activeTabRight = tab ? tab.path : path;
      else this.activeTabLeft = tab ? tab.path : path;
    },
    setLayoutMode(mode) {
      if (mode === this.layoutMode) return;
      if (mode === 'single') {
        // 收起分屏：保留左侧；右侧标签若不在左侧则其内容条目随之关闭。
        const rightPaths = [...this.rightTabPaths];
        this.rightTabPaths = [];
        this.activeTabRight = null;
        this.layoutMode = 'single';
        this.activeRegion = 'left';
        rightPaths.forEach((p) => this.pruneOrphanTab(p));
        this.normalizeSplitLayout();
        const focusLeft = this.activeTabLeft || this.leftTabPaths[0] || null;
        this.setActiveTab(focusLeft, 'left');
        return;
      }
      // 从单屏进入分屏：等价于把当前激活文件复制到另一侧（同文件分屏）。
      if (this.layoutMode === 'single') {
        this.splitCurrentTab(mode);
        return;
      }
      // 分屏间切换朝向（左右 ↔ 上下）。
      this.layoutMode = mode;
    },

    setActiveRegion(region) {
      const next = region === 'right' ? 'right' : 'left';
      if (this.layoutMode === 'single') { this.activeRegion = 'left'; return; }
      this.activeRegion = next;
      // 切换激活侧时同步 activeTab/selectedFile 到该侧当前标签，驱动“新文件去激活侧”。
      const path = next === 'right' ? this.activeTabRight : this.activeTabLeft;
      if (path) this.setActiveTab(path, next);
    },

    // 把当前激活文件复制到另一侧 pane（同一文件两侧同时打开），并进入分屏。
    splitCurrentTab(mode) {
      const active = this.activeTab;
      if (!active || this.openTabs.length === 0) return;
      const normalizedMode = mode === 'horizontal' ? 'horizontal' : 'vertical';

      if (this.layoutMode === 'single') {
        // 左侧保持当前所有标签与激活；右侧放入同一文件并聚焦右侧。
        this.layoutMode = normalizedMode;
        this.addPathToPane('left', active);
        if (!this.activeTabLeft) this.activeTabLeft = active;
        this.rightTabPaths = [active];
        this.activeTabRight = active;
        this.setActiveTab(active, 'right');
        return;
      }

      // 已分屏：把当前激活侧的文件复制到另一侧并聚焦另一侧；同时按需切换朝向。
      this.layoutMode = normalizedMode;
      const from = this.activeRegion || 'left';
      const to = this.otherRegion(from);
      this.addPathToPane(to, active);
      this.setActiveTab(active, to);
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
