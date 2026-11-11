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

/**
 * Markdown 格式化选项的默认值。
 *
 * 必须与后端 `default_markdown_format_options()` 逐字段一致——那是老用户升级后实际会用到的值。
 * 前端留一份是为了首帧（配置还没 invoke 回来）时格式化不会用错参数。
 * 注意这组值（折行 80、清理行尾空白开启）与 `MarkdownFormatOptions::default()` 不同，
 * 改之前先想清楚会不会把已有文档重排。
 */
export const DEFAULT_MARKDOWN_FORMAT_OPTIONS = {
  max_line_width: 80,
  indent_width: 2,
  list_indent: 2,
  format_tables: true,
  format_lists: true,
  preserve_code_blocks: true,
  remove_extra_blank_lines: true,
  fix_extra_whitespace: true,
}

/**
 * 主编辑器（Monaco）显示配置的默认值。
 *
 * 必须与后端 `EditorDisplayConfig` 的 `default_editor_*` 逐字段一致——那是老用户升级后实际会用到的值；
 * 前端留一份是为了首帧（配置还没 invoke 回来）建编辑器时不会用错字号。
 * 取值刻意等于 GuaxiEditor 原先写死的 `fontSize: 18` 与 `minimap.enabled: false`。
 */
export const DEFAULT_EDITOR_DISPLAY_OPTIONS = {
  font_family: "'Maple Mono NF CN','JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', 'Courier New'",
  font_size: 18,
  minimap_enabled: false,
}

/** 主编辑器字号区间，与后端 EDITOR_FONT_SIZE_MIN / MAX 对齐。 */
export const EDITOR_FONT_SIZE_MIN = 8
export const EDITOR_FONT_SIZE_MAX = 48

/**
 * Markdown 图片显示配置的默认值，与后端 PreviewImageConfig::default() 对齐。
 *
 * 860 / 12 / 不显示图注 = 此前写死在预览与探索者样式里的那一组，
 * 所以升级后没动过设置的用户看到的图片和以前完全一样。
 */
export const DEFAULT_PREVIEW_IMAGE_OPTIONS = {
  max_width: 860,
  radius: 12,
  show_caption: false,
}

/** 与后端 normalize_preview_image_config 的夹取区间保持一致。 */
export const PREVIEW_IMAGE_MAX_WIDTH_MIN = 320
export const PREVIEW_IMAGE_MAX_WIDTH_MAX = 1400
export const PREVIEW_IMAGE_RADIUS_MAX = 28

/** 图片宽度上限/圆角落到 CSS 变量上的名字：样式里只认这两个变量，不认具体数值。 */
export const PREVIEW_IMAGE_MAX_WIDTH_VAR = '--guaxi-figure-max-width'
export const PREVIEW_IMAGE_RADIUS_VAR = '--guaxi-figure-radius'

/**
 * 主编辑器字体链的中文/缺字回退段。
 *
 * 这里的 Maple Mono NF CN 是必需的，不是随手加的：它的中文字形宽 1.2em、拉丁 0.6em，
 * 正好是 2 格；而源码编辑态的表格对齐（GuaxiEditor.formatMarkdownTablesInText）
 * 也是按「中文 = 2 格」补空格的。换成比例字体回退（例如微软雅黑中文 1.0em）后，
 * 中文行会逐字比对齐预期窄，表格竖线就会看起来对不齐。
 */
export const EDITOR_FONT_FALLBACK_STACK =
  "'Maple Mono NF CN', Consolas, monospace, 'Microsoft YaHei', 'Courier New'"

/**
 * 把配置里的字体值解析成真正交给 CSS / Monaco 的 font-family。
 *
 * 单字体值（旧版本存下的 `'MesloLGSDZ Nerd Font'`，或用户手输的一个名字）补上回退链；
 * 已经是逗号分隔字体链的原样返回。设置页预览和编辑器都用这一个函数，保证所见即所得。
 */
export const resolveEditorFontFamily = (fontFamily) => {
  const raw = String(fontFamily || '').trim()
  if (!raw) return DEFAULT_EDITOR_DISPLAY_OPTIONS.font_family
  if (raw.includes(',')) return raw
  return `${raw}, ${EDITOR_FONT_FALLBACK_STACK}`
}

/**
 * 主编辑器行高 = 字号 × 4/3。
 *
 * 4/3 正好等于 GuaxiEditor 原先写死的 18px 字号 / 24px 行高，所以默认字号下行高不变；
 * 行高写死成一个常数的话，用户把字号调到 24 就会开始压字。
 * 设置页的预览和编辑器本身都用这一个函数，避免预览与实机对不上。
 */
export const resolveEditorLineHeight = (fontSize) => {
  const size = Number(fontSize)
  const safe = Number.isFinite(size) && size > 0 ? size : DEFAULT_EDITOR_DISPLAY_OPTIONS.font_size
  return Math.round((safe * 4) / 3)
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
    // Markdown 格式化选项。以前写死在 GuaxiEditor 的 invokeMarkdownFormat 里，用户改不了。
    markdownFormat: { ...DEFAULT_MARKDOWN_FORMAT_OPTIONS },
    markdownFormatLoaded: false,
    // 主编辑器（Monaco）的字体、字号与小地图开关。键名沿用后端 config 的字段名，
    // 与 markdownFormat 一样不做驼峰转换，省掉一层映射。
    editorDisplay: { ...DEFAULT_EDITOR_DISPLAY_OPTIONS },
    editorDisplayLoaded: false,
    // Markdown 图片的宽度上限、圆角与图注开关。宽度/圆角通过 CSS 变量下发，
    // 图注开关由预览与探索者的媒体组增强逻辑直接读取。
    previewImage: { ...DEFAULT_PREVIEW_IMAGE_OPTIONS },
    previewImageLoaded: false,
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

    async loadMarkdownFormatConfig() {
      if (this.markdownFormatLoaded) return
      try {
        const loaded = await invoke('get_markdown_format_options')
        if (loaded && typeof loaded === 'object') {
          // 后端读到的是落盘值；合并而不是整体替换，这样将来后端新增字段时前端不会把它丢掉。
          this.markdownFormat = { ...DEFAULT_MARKDOWN_FORMAT_OPTIONS, ...loaded }
        }
      } catch (error) {
        console.warn('[SettingsStore] load markdown format options failed, keep defaults:', error)
      }
      this.markdownFormatLoaded = true
    },

    /** 局部更新某项格式化选项并落盘；调用方只传要改的字段。 */
    async setMarkdownFormatOption(key, value) {
      const next = { ...this.markdownFormat, [key]: value }
      this.markdownFormat = next
      this.markdownFormatLoaded = true
      try {
        await invoke('set_markdown_format_options', { options: next })
      } catch (error) {
        console.error('[SettingsStore] markdown format options persist failed:', error)
        throw error
      }
    },

    async loadEditorDisplayConfig() {
      if (this.editorDisplayLoaded) return
      try {
        const loaded = await invoke('get_editor_display_config')
        if (loaded && typeof loaded === 'object') {
          // 与 markdownFormat 同一套写法：合并而不是整体替换，后端将来新增字段时前端不会丢。
          this.editorDisplay = { ...DEFAULT_EDITOR_DISPLAY_OPTIONS, ...loaded }
        }
      } catch (error) {
        console.warn('[SettingsStore] load editor display config failed, keep defaults:', error)
      }
      this.editorDisplayLoaded = true
    },

    /**
     * 局部更新主编辑器显示配置并落盘；调用方只传要改的字段。
     *
     * 先改 store（编辑器上的 watcher 会立刻 updateOptions，所见即所得），再写配置文件；
     * 后端会把字号夹到合法区间后回传，所以最后用返回的那份覆盖本地值——
     * 否则输入框里会留着 200 而编辑器实际用的是 48。
     */
    async setEditorDisplayOption(key, value) {
      const next = { ...this.editorDisplay, [key]: value }
      this.editorDisplay = next
      this.editorDisplayLoaded = true
      try {
        const saved = await invoke('set_editor_display_config', {
          editorDisplayConfig: next,
        })
        if (saved && typeof saved === 'object') {
          this.editorDisplay = { ...DEFAULT_EDITOR_DISPLAY_OPTIONS, ...saved }
        }
      } catch (error) {
        console.error('[SettingsStore] editor display config persist failed:', error)
        throw error
      }
    },

    async loadPreviewImageConfig() {
      if (this.previewImageLoaded) {
        this.applyPreviewImageVars()
        return
      }
      try {
        const loaded = await invoke('get_preview_image_config')
        if (loaded && typeof loaded === 'object') {
          // 与 editorDisplay 同一套写法：合并而不是整体替换，后端新增字段时不会丢。
          this.previewImage = { ...DEFAULT_PREVIEW_IMAGE_OPTIONS, ...loaded }
        }
      } catch (error) {
        console.warn('[SettingsStore] load preview image config failed, keep defaults:', error)
      }
      this.previewImageLoaded = true
      this.applyPreviewImageVars()
    },

    /**
     * 局部更新图片显示配置并落盘。
     *
     * 先改 store 并立刻把 CSS 变量刷到根节点（图片尺寸即时变化），再写配置文件；
     * 后端会夹取区间后回传，所以最后用返回的那份覆盖本地值。
     */
    async setPreviewImageOption(key, value) {
      const next = { ...this.previewImage, [key]: value }
      this.previewImage = next
      this.previewImageLoaded = true
      this.applyPreviewImageVars()
      try {
        const saved = await invoke('set_preview_image_config', {
          previewImageConfig: next,
        })
        if (saved && typeof saved === 'object') {
          this.previewImage = { ...DEFAULT_PREVIEW_IMAGE_OPTIONS, ...saved }
          this.applyPreviewImageVars()
        }
      } catch (error) {
        console.error('[SettingsStore] preview image config persist failed:', error)
        throw error
      }
    },

    /**
     * 把宽度上限与圆角写成根节点上的 CSS 变量。
     *
     * 样式里所有图片尺寸都走 var(--guaxi-figure-max-width, 860px) 这类写法，
     * 所以这里改一次全局生效，不用去碰预览、探索者、阅读器各自的样式表。
     * 幂等，可重复调用。
     */
    applyPreviewImageVars() {
      const root = typeof document !== 'undefined' ? document.documentElement : null
      if (!root?.style) return
      const maxWidth = Number(this.previewImage?.max_width)
      const radius = Number(this.previewImage?.radius)
      const safeMaxWidth =
        Number.isFinite(maxWidth) && maxWidth > 0 ? maxWidth : DEFAULT_PREVIEW_IMAGE_OPTIONS.max_width
      const safeRadius =
        Number.isFinite(radius) && radius >= 0 ? radius : DEFAULT_PREVIEW_IMAGE_OPTIONS.radius
      root.style.setProperty(PREVIEW_IMAGE_MAX_WIDTH_VAR, `${safeMaxWidth}px`)
      root.style.setProperty(PREVIEW_IMAGE_RADIUS_VAR, `${safeRadius}px`)
      // 图注开关只切一个根节点类：媒体组增强阶段已经把 alt 写进 data-gx-caption，
      // 这里加/删类即可让图注显隐，不需要为了一个开关重渲染预览或探索者面板。
      root.classList.toggle('guaxi-show-figure-caption', !!this.previewImage?.show_caption)
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
