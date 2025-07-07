// src/stores/settingsStore.js
import { defineStore } from "pinia";
import { invoke } from '@tauri-apps/api/core'

export const useSettingsStore = defineStore("settings", {
  state: () => ({
    selectedMenu: "笔记库", // 默认选中的菜单项
    librariesRoot: "",
  }),
  actions: {
    setSelectedMenu(menu) {
      this.selectedMenu = menu;
    },

    set_librariesRoot_settings(path) {
      this.librariesRoot = path;
    },

    async get_librariesRoot() {
      try {
        this.librariesRoot = await invoke("get_libraries_root");
        // console.log("初始化: 获取当librariesRoot路径成功", this.librariesRoot);
      } catch (error) {
        // console.error("获取当librariesRoot仓库路径失败", error);
      }
    },
  },
});
