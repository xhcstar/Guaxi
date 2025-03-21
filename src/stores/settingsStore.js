// src/stores/settingsStore.js
import { defineStore } from 'pinia';

export const useSettingsStore = defineStore('settings', {
  state: () => ({
    selectedMenu: '笔记库', // 默认选中的菜单项
    librariesRoot: "",
  }),
  actions: {
    setSelectedMenu(menu) {
      this.selectedMenu = menu;
    },
  },
});