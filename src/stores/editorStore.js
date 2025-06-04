import { defineStore } from 'pinia';

export const useEditorStore = defineStore('editor', {
  state: () => ({
    currentFile: null,
    highlightInfo: null,
    decorations: [] // 存储当前高亮装饰器
  }),
  actions: {
    // 设置当前文件和高亮信息
    setFile(fileData) {
      this.currentFile = {
        path: fileData.path,
        content: fileData.content
      };
      
      if (fileData.highlightInfo) {
        this.highlightInfo = fileData.highlightInfo;
      }
      
      this.triggerEditorUpdate();
    },
    
    // 清除高亮
    clearHighlights() {
      this.highlightInfo = null;
      this.decorations = [];
      this.triggerEditorUpdate();
    },
    
    triggerEditorUpdate() {
      // 实际更新逻辑在编辑器组件中
    }
  }
});