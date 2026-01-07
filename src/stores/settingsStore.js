// src/stores/settingsStore.js
import { defineStore } from "pinia";
import { invoke } from '@tauri-apps/api/core'

export const useSettingsStore = defineStore("settings", {
  state: () => ({
    selectedMenu: "noterepo",
    librariesRoot: "",
    knowledgeBasesRoot: "",
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
      if (!document?.documentElement) return
      document.documentElement.dataset.guaxiTheme = t
      document.documentElement.classList.toggle('dark-mode', t === 'dark')
    },

    async loadAppearanceConfig() {
      if (this.appearanceLoaded) {
        this.applyAppearanceTheme(this.appearanceTheme)
        return
      }

      const themeParam = (() => {
        try {
          let qs = window.location.search || ''
          if (!qs) {
            const hash = window.location.hash || ''
            const qIdx = hash.indexOf('?')
            if (qIdx >= 0) qs = hash.slice(qIdx)
          }
          const t = new URLSearchParams(qs).get('theme')
          return t === 'dark' || t === 'light' ? t : null
        } catch {
          return null
        }
      })()

      if (themeParam) {
        this.appearanceTheme = themeParam
        this.applyAppearanceTheme(this.appearanceTheme)
      }

      try {
        const appearance = await invoke('get_appearance_config')
        const theme = appearance?.theme === 'dark' ? 'dark' : 'light'
        const fontSize = Number(appearance?.font_size)

        this.appearanceTheme = themeParam || theme
        this.appearanceFontSize = Number.isFinite(fontSize) && fontSize > 0 ? fontSize : 14
        this.appearanceLoaded = true
        this.applyAppearanceTheme(this.appearanceTheme)
      } catch (error) {
        this.appearanceTheme = themeParam || 'light'
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
      } catch (error) {
      }
    },

    async get_knowledgeBasesRoot() {
      try {
        this.knowledgeBasesRoot = await invoke("get_knowledge_bases_root");
      } catch (error) {
      }
    },
  },
});
