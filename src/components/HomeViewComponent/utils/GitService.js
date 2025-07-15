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
    return await invoke("get_file_status", { path: filePath });
  }

  async getLineChanges(filePath) {
    return await invoke("get_line_changes", { path: filePath });
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
