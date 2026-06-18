import { defineStore } from "pinia";
import { createLogger } from "../utils/logger";

const logger = createLogger("editorBridgeStore");

const normalizePathKey = (path) => String(path || "").replace(/\\/g, "/");

const normalizeContent = (content) => String(content ?? "").replace(/\r\n?/g, "\n");

const sameMeta = (a, b) => {
  if (!a || !b) return false;
  return String(a.mtimeMs ?? "") === String(b.mtimeMs ?? "") &&
    Number(a.fileSize ?? -1) === Number(b.fileSize ?? -2);
};

export const useEditorBridgeStore = defineStore("editorBridge", {
  state: () => ({
    // pathKey -> owner。没有记录时默认 Monaco 拥有写权限。
    owners: {},
    // pathKey -> { mtimeMs, fileSize }，作为写入前冲突检测快照。
    fileMetaSnapshots: {},
    // pathKey -> 最近一次检测到的冲突说明。
    conflicts: {},
  }),

  getters: {
    getOwner: (state) => (path) => state.owners[normalizePathKey(path)] || "monaco",
    isMonacoReadonly: (state) => (path) => state.owners[normalizePathKey(path)] === "neovim",
  },

  actions: {
    // 设计思路：
    // 1. Monaco 和 Neovim 不直接抢写同一个文件，owner 是唯一写权限来源。
    // 2. mtime + size 是轻量冲突哨兵：写入前发现磁盘变化，就先要求同步，不做覆盖写。
    // 3. Neovim 保存属于外部磁盘变化，Guaxi 只在旁观状态下自动吸收；Monaco dirty 时保留冲突提示。
    setOwner(path, owner, reason = "") {
      const key = normalizePathKey(path);
      if (!key) return;
      const next = owner === "neovim" ? "neovim" : "monaco";
      this.owners = { ...this.owners, [key]: next };
      logger.info("[EditorBridge] owner updated", { path, owner: next, reason });
    },

    clearOwner(path, reason = "") {
      const key = normalizePathKey(path);
      if (!key) return;
      const next = { ...this.owners };
      delete next[key];
      this.owners = next;
      logger.info("[EditorBridge] owner cleared", { path, reason });
    },

    setSnapshot(path, meta) {
      const key = normalizePathKey(path);
      if (!key || !meta) return;
      this.fileMetaSnapshots = {
        ...this.fileMetaSnapshots,
        [key]: {
          mtimeMs: meta.mtimeMs ?? meta.mtime_ms ?? null,
          fileSize: Number(meta.fileSize ?? meta.file_size ?? 0),
        },
      };
      logger.debug("[EditorBridge] snapshot updated", {
        path,
        snapshot: this.fileMetaSnapshots[key],
      });
    },

    clearConflict(path) {
      const key = normalizePathKey(path);
      if (!key || !this.conflicts[key]) return;
      const next = { ...this.conflicts };
      delete next[key];
      this.conflicts = next;
    },

    setConflict(path, message, detail = {}) {
      const key = normalizePathKey(path);
      if (!key) return;
      this.conflicts = {
        ...this.conflicts,
        [key]: {
          message,
          detail,
          at: Date.now(),
        },
      };
      logger.warn("[EditorBridge] conflict detected", { path, message, detail });
    },

    async trackFile(path, fileTreeStore) {
      if (!path || !fileTreeStore?.fetchFileMeta) return;
      const meta = await fileTreeStore.fetchFileMeta(path);
      if (meta?.ok) this.setSnapshot(path, meta);
    },

    async prepareWrite(path, writer, fileTreeStore) {
      const key = normalizePathKey(path);
      if (!key) return true;

      const owner = this.getOwner(path);
      const normalizedWriter = writer === "neovim" ? "neovim" : "monaco";
      if (owner !== normalizedWriter) {
        const message = owner === "neovim"
          ? "当前文件正在由 Neovim 编辑，Guaxi 已进入旁观状态"
          : "当前文件正在由 Guaxi 编辑，Neovim 应保持旁观状态";
        this.setConflict(path, message, { owner, writer: normalizedWriter });
        throw new Error(message);
      }

      if (!fileTreeStore?.fetchFileMeta) return true;
      const latest = await fileTreeStore.fetchFileMeta(path);
      if (!latest?.ok) return true;

      const snapshot = this.fileMetaSnapshots[key];
      if (snapshot && !sameMeta(snapshot, latest)) {
        const message = "文件在磁盘上已经变化，请先同步其他编辑器的修改后再写入";
        this.setConflict(path, message, { snapshot, latest, writer: normalizedWriter });
        throw new Error(message);
      }

      return true;
    },

    async markAfterWrite(path, fileTreeStore) {
      this.clearConflict(path);
      await this.trackFile(path, fileTreeStore);
    },

    async absorbExternalChange(path, fileTreeStore) {
      const key = normalizePathKey(path);
      if (!key || !fileTreeStore?.openTabs) return { status: "ignored" };

      const tab = fileTreeStore.openTabs.find((item) => normalizePathKey(item?.path) === key);
      if (!tab || tab.unsupported) return { status: "ignored" };

      const owner = this.getOwner(path);
      if (owner !== "neovim" && tab.dirty) {
        const message = "磁盘文件已变化，但 Guaxi 中仍有未保存修改，已阻止自动覆盖";
        this.setConflict(path, message, { owner, dirty: true });
        return { status: "conflict", message };
      }

      const res = await fileTreeStore.fetchFileContent(tab.path);
      if (!res?.ok) return { status: "failed", message: res?.code || "READ_FAILED" };

      const nextContent = normalizeContent(res.content);
      tab.content = nextContent;
      tab.savedContent = nextContent;
      tab.dirty = false;
      tab.preview = fileTreeStore.generatePreview(nextContent);

      if (fileTreeStore.selectedFile && normalizePathKey(fileTreeStore.selectedFile.path) === key) {
        fileTreeStore.selectedFile = {
          ...fileTreeStore.selectedFile,
          content: nextContent,
        };
      }

      await this.trackFile(tab.path, fileTreeStore);
      this.clearConflict(tab.path);
      logger.info("[EditorBridge] absorbed external file change", {
        path: tab.path,
        owner,
        length: nextContent.length,
      });
      return { status: "synced", path: tab.path };
    },
  },
});
