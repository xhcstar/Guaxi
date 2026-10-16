// src/stores/settingsStore.js
import { defineStore } from "pinia";
import { invoke } from '@tauri-apps/api/core'
import { createLogger } from "../utils/logger"
import { applyCodeThemeToDocument, DEFAULT_CODE_THEME_ID, normalizeCodeThemeId } from '../monaco/codeThemes'
import { reapplyGuaxiMonacoCodeTheme } from '../monaco/guaxiMonacoThemes'

const logger = createLogger("settingsStore")
const console = {
  debug: (...args) => logger.debug(...args),
  log: (...args) => logger.info(...args),
  info: (...args) => logger.info(...args),
  warn: (...args) => logger.warn(...args),
  error: (...args) => logger.error(...args),
}

export const useSettingsStore = defineStore("settings", {
  state: () => ({
    selectedMenu: "noterepo",
    librariesRoot: "",
    knowledgeBasesRoot: "",
    appearanceTheme: "light",
    appearanceFontSize: 14,
    appearanceLoaded: false,
    // 代码配色主题（编辑器语法色 + Markdown 代码块配色共用同一套）
    codeTheme: DEFAULT_CODE_THEME_ID,
    codeThemeLoaded: false,
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
      // 代码主题的明暗两套变量是随主题一起注入的，切明暗时只需把 Markdown 代码块那张样式表
      // 换成对应版本，并让 Monaco 重新应用一次主题。
      this.applyCodeTheme()
    },

    /** 把当前代码主题落到文档（`.hljs` 样式表）并让 Monaco 生效。幂等，可重复调用。 */
    applyCodeTheme() {
      applyCodeThemeToDocument(this.codeTheme, this.appearanceTheme)
      reapplyGuaxiMonacoCodeTheme(this.codeTheme, this.appearanceTheme)
    },

    async loadCodeThemeConfig() {
      if (this.codeThemeLoaded) {
        this.applyCodeTheme()
        return
      }
      try {
        this.codeTheme = normalizeCodeThemeId(await invoke('get_code_theme'))
      } catch (error) {
        console.warn('[SettingsStore] load code theme failed, fall back to default:', error)
        this.codeTheme = DEFAULT_CODE_THEME_ID
      }
      this.codeThemeLoaded = true
      this.applyCodeTheme()
    },

    async setCodeTheme(id) {
      const next = normalizeCodeThemeId(id)
      this.codeTheme = next
      this.codeThemeLoaded = true
      this.applyCodeTheme()

      console.log('[SettingsStore] apply code theme:', { next })

      try {
        await invoke('set_code_theme', { codeTheme: next })
      } catch (error) {
        console.error('[SettingsStore] code theme persist failed:', error)
        throw error
      }
    },

    async loadAppearanceConfig() {
      if (this.appearanceLoaded) {
        this.applyAppearanceTheme(this.appearanceTheme)
        await this.loadCodeThemeConfig()
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
        const fontSize = Number(appearance?.font_size ?? appearance?.fontSize)

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

      await this.loadCodeThemeConfig()
    },

    async setAppearanceTheme(theme) {
      const next = theme === 'dark' ? 'dark' : 'light'
      const prev = this.appearanceTheme

      this.appearanceTheme = next
      this.appearanceLoaded = true
      this.applyAppearanceTheme(this.appearanceTheme)

      console.log('[SettingsStore] apply theme:', { prev, next })

      try {
        await invoke('set_appearance_config', {
          theme: this.appearanceTheme,
          fontSize: this.appearanceFontSize,
        })
        console.log('[SettingsStore] theme persisted:', {
          theme: this.appearanceTheme,
          font_size: this.appearanceFontSize,
        })
      } catch (error) {
        console.error('[SettingsStore] theme persist failed:', error)
        throw error
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
