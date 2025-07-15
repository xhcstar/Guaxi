// src/utils/GitService.js
import { invoke } from '@tauri-apps/api/core';

export default class GitService {
  constructor(workspacePath) {
    this.workspacePath = workspacePath;
  }

  async getCurrentBranch() {
    return await invoke("get_current_branch", { path: this.workspacePath });
  }

  async getFileStatus(filePath) {
    try {
      // 修改为调用 get_git_status
      return await invoke("get_git_status", { path: filePath });
    } catch (error) {
      console.error('获取文件状态失败:', error);
      return [];
    }
  }

  async getLineChanges(filePath) {
    try {
      // 这个命令在后端可能不存在，我们可以复用 get_git_status
      const status = await this.getFileStatus(filePath);
      return {
        added: status.filter(s => s.status === 'A').map(s => s.lines_added).flat(),
        modified: status.filter(s => s.status === 'M').map(s => s.lines_modified).flat()
      };
    } catch (error) {
      console.error('获取行变更失败:', error);
      return { added: [], modified: [] };
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
}