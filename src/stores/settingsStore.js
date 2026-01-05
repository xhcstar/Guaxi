// src/stores/settingsStore.js
import { defineStore } from "pinia";
import { invoke } from '@tauri-apps/api/core'

export const useSettingsStore = defineStore("settings", {
  state: () => ({
    selectedMenu: "noterepo",
    librariesRoot: "",
    appearanceTheme: "light",
    appearanceFontSize: 14,
    appearanceLoaded: false,
  }),
  actions: {
    setSelectedMenu(menu) {
      this.selectedMenu = menu;
    },

    set_librariesRoot_settings(path) {
      this.librariesRoot = path;
    },

    applyAppearanceTheme(theme) {
      const t = theme === 'dark' ? 'dark' : 'light'
      document.documentElement.dataset.guaxiTheme = t
      document.documentElement.classList.toggle('dark-mode', t === 'dark')
    },

    async loadAppearanceConfig() {
      if (this.appearanceLoaded) {
        this.applyAppearanceTheme(this.appearanceTheme)
        return
      }

      try {
        const appearance = await invoke('get_appearance_config')
        const theme = appearance?.theme === 'dark' ? 'dark' : 'light'
        const fontSize = Number(appearance?.font_size)

        this.appearanceTheme = theme
        this.appearanceFontSize = Number.isFinite(fontSize) && fontSize > 0 ? fontSize : 14
        this.appearanceLoaded = true
        this.applyAppearanceTheme(this.appearanceTheme)
      } catch (error) {
        this.appearanceTheme = 'light'
        this.appearanceFontSize = 14
        this.appearanceLoaded = true
        this.applyAppearanceTheme(this.appearanceTheme)
      }
    },

    async setAppearanceTheme(theme) {
      const next = theme === 'dark' ? 'dark' : 'light'
      this.appearanceTheme = next
      this.applyAppearanceTheme(this.appearanceTheme)

      try {
        await invoke('set_appearance_config', {
          theme: this.appearanceTheme,
          font_size: this.appearanceFontSize,
        })
      } catch (error) {
      }
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
