import { defineStore } from "pinia";
import * as monaco from "monaco-editor";
import { markRaw } from "vue";
import { createLogger } from "../utils/logger";

const logger = createLogger("editorStore");
const console = {
  debug: (...args) => logger.debug(...args),
  log: (...args) => logger.info(...args),
  info: (...args) => logger.info(...args),
  warn: (...args) => logger.warn(...args),
  error: (...args) => logger.error(...args),
};

export const useEditorStore = defineStore("editor", {
  state: () => ({
    currentFile: null, // 当前打开的文件信息
    highlightInfo: null, // 高亮相关信息
    decorations: [], // 存储当前高亮装饰器
    editorInstance: null, // Monaco编辑器实例引用
    isInserting: false, // 新增：插入操作状态标志

    autoSaveEnabled: true,
    autoSaveIntervalMs: 5 * 60 * 1000,
    lastAutoSaveAt: null,

    // 状态栏相关状态
    cursorPosition: { lineNumber: 1, column: 1 },
    language: "plaintext",
    problems: { errors: 0, warnings: 0 },
    encoding: "UTF-8",
    lineEnding: "LF",
    indentation: "Spaces: 2",
    selectionStatus: null,

    // Git 相关状态
    gitDecorations: [], // Git 行变更装饰器定义
    gitDecorationIds: [], // 当前应用的装饰器ID
    gitChanges: {
      // Git 变更统计
      added: 0,
      modified: 0,
      deleted: 0,
    },
    gitBranch: "", // 当前 Git 分支

    // Home 编辑器预览模式：用于页面切换/组件重建后恢复（edit/preview/split/realtime）
    homePreviewModeKey: "edit",
  }),
  actions: {
    // 检查文件是否处于激活状态
    isFileActive(file) {
      return this.activeFile?.path === file?.path;
    },

    // 设置当前文件和高亮信息
    setFile(fileData) {
      this.currentFile = {
        path: fileData.path,
        content: fileData.content,
      };

      if (fileData.highlightInfo) {
        this.highlightInfo = fileData.highlightInfo;
      }

      this.triggerEditorUpdate();
    },

    // 清除所有装饰器（包括 Git 装饰器）
    clearHighlights() {
      this.highlightInfo = null;
      this.decorations = [];
      this.clearGitDecorations();
      this.triggerEditorUpdate();
    },

    triggerEditorUpdate() {
      if (this.editorInstance) {
        // 强制重新渲染编辑器
        this.editorInstance.layout();
      }
      console.debug("[EditorStore] Triggering editor update");
    },

    setHomePreviewModeKey(key) {
      const k = String(key || "").trim().toLowerCase();
      const next = ["edit", "preview", "split", "realtime"].includes(k) ? k : "edit";
      if (this.homePreviewModeKey === next) return;

      this.homePreviewModeKey = next;
      console.info("[EditorStore] homePreviewModeKey updated", { key: next });
    },

    // Git 相关操作
    setGitBranch(branch) {
      this.gitBranch = branch;
    },

    setGitChanges(changes) {
      this.gitChanges = {
        added: changes.added || 0,
        modified: changes.modified || 0,
        deleted: changes.deleted || 0,
      };
    },

    setGitDecorations(decorations) {
      this.gitDecorations = decorations;
      console.debug('[GitDecorations] setGitDecorations', {
        count: Array.isArray(decorations) ? decorations.length : null
      });
      this.applyGitDecorations();
    },

    applyGitDecorations() {
      if (!this.editorInstance) {
        console.debug('[GitDecorations] applyGitDecorations skipped (no editorInstance)');
        return;
      }

      const hasDecorations = Array.isArray(this.gitDecorations) && this.gitDecorations.length > 0;
      if (!hasDecorations) {
        if (Array.isArray(this.gitDecorationIds) && this.gitDecorationIds.length > 0) {
          this.editorInstance.deltaDecorations(this.gitDecorationIds, []);
        }
        this.gitDecorationIds = [];
        console.debug('[GitDecorations] applyGitDecorations cleared (no decorations)', {
          decorationsType: typeof this.gitDecorations,
          count: Array.isArray(this.gitDecorations) ? this.gitDecorations.length : null
        });
        return;
      }

      try {
        if (Array.isArray(this.gitDecorationIds) && this.gitDecorationIds.length > 0) {
          this.editorInstance.deltaDecorations(this.gitDecorationIds, []);
        }

        this.gitDecorationIds = this.editorInstance.deltaDecorations([], this.gitDecorations);

        console.debug('[GitDecorations] applied', {
          decorationsCount: this.gitDecorations.length,
          decorationIdsCount: Array.isArray(this.gitDecorationIds) ? this.gitDecorationIds.length : null
        });
      } catch (e) {
        console.error('[GitDecorations] apply failed:', e);
      }
    },

    clearGitDecorations() {
      const before = Array.isArray(this.gitDecorationIds) ? this.gitDecorationIds.length : null;
      if (this.editorInstance && Array.isArray(this.gitDecorationIds) && this.gitDecorationIds.length > 0) {
        this.editorInstance.deltaDecorations(this.gitDecorationIds, []);
      }
      this.gitDecorations = [];
      this.gitDecorationIds = [];
      console.debug('[GitDecorations] cleared', { before });
    },

    updateGitLineDecorations({ added = [], modified = [], deleted = [] }) {
      console.debug('[GitDecorations] updateGitLineDecorations input', {
        added: Array.isArray(added) ? added.length : null,
        modified: Array.isArray(modified) ? modified.length : null,
        deleted: Array.isArray(deleted) ? deleted.length : null
      });

      const decorations = [
        ...(Array.isArray(added) ? added : []).map((line) => this.createGitDecoration(line, "added")),
        ...(Array.isArray(modified) ? modified : []).map((line) => this.createGitDecoration(line, "modified")),
        ...(Array.isArray(deleted) ? deleted : []).map((line) => this.createGitDecoration(line, "deleted")),
      ];

      console.debug('[GitDecorations] updateGitLineDecorations generated', {
        count: decorations.length
      });

      this.setGitDecorations(decorations);
    },

    createGitDecoration(line, type) {
      const className = `git-line-${type}`;
      const hoverMessage = {
        added: "新增行",
        modified: "修改行",
        deleted: "删除行",
      }[type];

      return {
        range: new monaco.Range(line, 1, line, 1),
        options: {
          isWholeLine: true,
          linesDecorationsClassName: className,
          hoverMessage: { value: hoverMessage },
          stickiness:
            monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges,
        },
      };
    },

    // 更新设置编辑器实例（确保清除旧装饰器）
    setEditorInstance(editor) {
      if (this.editorInstance && this.gitDecorationIds.length > 0) {
        this.editorInstance.deltaDecorations(this.gitDecorationIds, []);
      }

      this.editorInstance = editor ? markRaw(editor) : null;

      if (editor && this.gitDecorations.length > 0) {
        this.applyGitDecorations();
      }
    },

    async insertContent(content) {
      if (!this.editorInstance?.getModel()) return false;

      try {
        this.isInserting = true; // 标记正在插入

        // 获取当前光标位置
        const position = this.editorInstance.getPosition();

        // 执行插入操作
        this.editorInstance.executeEdits("insert-content", [
          {
            range: new monaco.Range(
              position.lineNumber,
              position.column,
              position.lineNumber,
              position.column
            ),
            text: content,
            forceMoveMarkers: true,
          },
        ]);

        // 计算新光标位置
        const lines = content.split("\n");
        let newLineNumber = position.lineNumber;
        let newColumn = position.column;

        if (lines.length > 1) {
          newLineNumber += lines.length - 1;
          newColumn = lines[lines.length - 1].length + 1;
        } else {
          newColumn += content.length;
        }

        // 设置新光标位置
        const newPosition = new monaco.Position(newLineNumber, newColumn);
        this.editorInstance.setPosition(newPosition);
        this.editorInstance.revealPositionInCenter(newPosition);

        // 创建新的选择范围
        const newSelection = new monaco.Selection(
          newLineNumber,
          newColumn,
          newLineNumber,
          newColumn
        );
        this.editorInstance.setSelection(newSelection);
        this.editorInstance.focus();

        return true;
      } catch (error) {
        console.error("Insert failed:", error);
        return false;
      } finally {
        this.isInserting = false; // 重置插入状态
      }
    },
    // 更新光标位置
    setCursorPosition(position) {
      this.cursorPosition = position;
    },

    setAutoSaveEnabled(enabled) {
      this.autoSaveEnabled = !!enabled;
    },

    setAutoSaveIntervalMs(ms) {
      const next = Number(ms);
      this.autoSaveIntervalMs = Number.isFinite(next) && next > 0 ? next : 5 * 60 * 1000;
    },

    setLastAutoSaveAt(ts) {
      const next = Number(ts);
      this.lastAutoSaveAt = Number.isFinite(next) && next > 0 ? next : Date.now();
    },

    // 更新问题计数
    setProblems({ errors = 0, warnings = 0 }) {
      this.problems = { errors, warnings };
    },

    // 更新语言
    setLanguage(language) {
      this.language = language;
      if (this.editorInstance) {
        const model = this.editorInstance.getModel();
        if (model) {
          monaco.editor.setModelLanguage(model, language);
        }
      }
    },

    // 更新编码
    setEncoding(encoding) {
      this.encoding = encoding;
    },

    // 更新行尾符
    setLineEnding(lineEnding) {
      this.lineEnding = lineEnding;
    },

    // 更新缩进
    setIndentation(indentation) {
      this.indentation = indentation;
    },

    // 更新选择状态
    setSelectionStatus(status) {
      this.selectionStatus = status;
    },
    // 更新光标位置
    updateCursorPosition(position) {
      this.cursorPosition = position;

      // 如果编辑器实例存在，确保光标可见
      if (this.editorInstance) {
        this.editorInstance.revealPositionInCenterIfOutsideViewport(position);
      }
    },

    // 更新语言模式
    updateLanguage(language) {
      this.language = language;

      // 如果编辑器实例存在，更新语言
      if (this.editorInstance) {
        const model = this.editorInstance.getModel();
        if (model) {
          monaco.editor.setModelLanguage(model, language);
        }
      }
    },

    // 更新问题计数
    updateProblemCount(errors = 0, warnings = 0) {
      this.problems = { errors, warnings };
    },

    // 更新编码信息
    updateEncoding(encoding) {
      this.encoding = encoding;

      // 这里可以添加实际编码转换逻辑
      console.log("Encoding changed to:", encoding);
    },

    // 更新行尾符
    updateLineEnding(lineEnding) {
      this.lineEnding = lineEnding;

      // 如果编辑器实例存在，更新行尾符
      if (this.editorInstance) {
        const model = this.editorInstance.getModel();
        if (model) {
          const options = model.getOptions();
          options.defaultEOL =
            lineEnding === "LF"
              ? monaco.editor.DefaultEndOfLine.LF
              : monaco.editor.DefaultEndOfLine.CRLF;
          model.updateOptions(options);
        }
      }
    },

    // 更新缩进设置
    updateIndentation(indentation) {
      this.indentation = indentation;

      // 如果编辑器实例存在，更新缩进
      if (this.editorInstance) {
        const model = this.editorInstance.getModel();
        if (model) {
          const options = model.getOptions();

          if (indentation === "Tabs") {
            options.insertSpaces = false;
            options.tabSize = 4;
          } else {
            options.insertSpaces = true;
            const spaceCount = parseInt(indentation.split(":")[1].trim());
            options.tabSize = spaceCount;
          }

          model.updateOptions(options);
        }
      }
    },

    // 更新选择状态
    updateSelectionStatus(status) {
      this.selectionStatus = status;
    },

    // 检测文件编码
    detectFileEncoding(content) {
      try {
        const result = detect(content);
        return result.encoding || "UTF-8";
      } catch {
        return "UTF-8";
      }
    },
  },
});
