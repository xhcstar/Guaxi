// src/utils/GitService.js
import { invoke } from "@tauri-apps/api/core";

export default class GitService {
  constructor(workspacePath) {
    this.workspacePath = workspacePath;
  }

  async getCurrentBranch() {
    return await invoke("get_current_branch", { path: this.workspacePath });
  }

  async getFileStatus(repoPath = this.workspacePath) {
    try {
      return await invoke("get_git_status", { path: repoPath });
    } catch (error) {
      console.error("获取文件状态失败:", error);
      return [];
    }
  }

  async commit(message) {
    return await invoke("git_commit", {
      path: this.workspacePath,
      message,
    });
  }

  async pull() {
    return await invoke("git_pull", { path: this.workspacePath });
  }

  async push() {
    return await invoke("git_push", { path: this.workspacePath });
  }

  async getLineChanges(filePath) {
    try {
      const status = await this.getFileStatus();
      const target = String(filePath || '').replace(/\\/g, '/');

      const fileStatus = status.filter((s) => {
        const p = String(s?.path || '').replace(/\\/g, '/');
        return p === target || (target && p.endsWith('/' + target));
      });

      return {
        added: fileStatus
          .filter((s) => s.status === "A")
          .flatMap((s) => s.lines_added),
        modified: fileStatus
          .filter((s) => s.status === "M")
          .flatMap((s) => s.lines_modified),
        deleted: fileStatus
          .filter((s) => s.status === "D")
          .flatMap((s) => s.lines_deleted),
      };
    } catch (error) {
      console.error("获取行变更失败:", error);
      return { added: [], modified: [], deleted: [] };
    }
  }
}
