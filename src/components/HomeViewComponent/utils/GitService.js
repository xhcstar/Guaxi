// src/utils/GitService.js
import { invoke } from "@tauri-apps/api/core";
import { createLogger } from '../../../utils/logger'

const logger = createLogger('GitService')
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
      logger.error("获取文件状态失败:", error);
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
      const targetAbs = String(filePath || '').replace(/\\/g, '/');
      const repoAbs = String(this.workspacePath || '').replace(/\\/g, '/');
      const targetRel = repoAbs && targetAbs.startsWith(repoAbs + '/')
        ? targetAbs.slice(repoAbs.length + 1)
        : targetAbs;

      const status = await this.getFileStatus(filePath);
      const list = Array.isArray(status) ? status : [];

      const matched = list.filter((s) => {
        const p = String(s?.path || '').replace(/\\/g, '/');
        return p === targetAbs || p === targetRel || (targetRel && p.endsWith('/' + targetRel));
      });

      logger.debug('[GitService] getLineChanges', {
        repoAbs,
        targetAbs,
        targetRel,
        statusCount: list.length,
        matchedCount: matched.length,
        sample: matched.slice(0, 2)
      });

      return {
        added: matched
          .filter((s) => s.status === 'A')
          .flatMap((s) => Array.isArray(s.lines_added) ? s.lines_added : []),
        modified: matched
          .filter((s) => s.status === 'M')
          .flatMap((s) => Array.isArray(s.lines_modified) ? s.lines_modified : []),
        deleted: matched
          .filter((s) => s.status === 'D')
          .flatMap((s) => Array.isArray(s.lines_deleted) ? s.lines_deleted : []),
      };
    } catch (error) {
      logger.error('获取行变更失败:', error);
      return { added: [], modified: [], deleted: [] };
    }
  }
}
