import { defineStore } from "pinia";

export const useRightSiderStore = defineStore("rightSider", {
  state: () => ({
    searchResults: {
      fileResults: [],
      noteResults: [],
      excerptResults: [], // 新增：摘录搜索结果
      searchQuery: "",
    },
    // 新增：当前右侧面板类型（如 'search', 'outline' 等）
    panel: "",
    // 新增：右侧边栏是否展开
    isOpen: false,
  }),
  actions: {
    setSearchResults(results) {
      this.searchResults = results;
    },
    clearSearchResults() {
      this.searchResults = {
        fileResults: [],
        noteResults: [],
        excerptResults: [],
        searchQuery: "",
      };
    },
    // 新增：设置当前面板类型
    setPanel(name) {
      this.panel = name;
    },
    // 新增：展开右侧边栏
    open() {
      this.isOpen = true;
    },
    // 新增：关闭右侧边栏
    close() {
      this.isOpen = false;
    },
  },
});
