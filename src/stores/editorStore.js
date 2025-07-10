import { defineStore } from "pinia";
import * as monaco from "monaco-editor";
import { markRaw } from "vue";

export const useEditorStore = defineStore("editor", {
  state: () => ({
    currentFile: null, // 当前打开的文件信息
    highlightInfo: null, // 高亮相关信息
    decorations: [], // 存储当前高亮装饰器
    editorInstance: null, // Monaco编辑器实例引用
    isInserting: false, // 新增：插入操作状态标志
    cursorPosition: { lineNumber: 1, column: 1 },
    language: 'plaintext',
    problems: { errors: 0, warnings: 0 },
    encoding: 'UTF-8',
    lineEnding: 'LF',
    indentation: 'Spaces: 2'
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

    // 清除所有高亮
    clearHighlights() {
      this.highlightInfo = null;
      this.decorations = [];
      this.triggerEditorUpdate();
    },

    triggerEditorUpdate() {
      if (this.editorInstance) {
        // 强制重新渲染编辑器
        this.editorInstance.layout();
      }
      console.debug("[EditorStore] Triggering editor update");
    },

    setEditorInstance(editor) {
      this.editorInstance = editor ? markRaw(editor) : null; // 保持非响应式
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

    setCursorPosition(position) {
      this.cursorPosition = position
    },
    setLanguage(language) {
      this.language = language
    },
    setProblems({ errors, warnings }) {
      this.problems = { errors, warnings }
    }
  },
});
