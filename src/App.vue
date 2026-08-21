<template>
  <n-config-provider :theme="naiveTheme" :theme-overrides="themeOverrides">
    <n-message-provider placement="bottom">
      <RouterView />
    </n-message-provider>
  </n-config-provider>
</template>

<script setup>
import { computed, onBeforeUnmount, onMounted } from 'vue'
import { RouterView } from 'vue-router'
import { NConfigProvider, NMessageProvider, darkTheme } from 'naive-ui'
import { useSettingsStore } from './stores/settingsStore'
import { mainNavigationItems } from './router/index.js'
import { GUAXI_SHORTCUTS, shortcutText, shortcutTooltip, useGuaxiHotkeys } from './composables/useGuaxiHotkeys'
import { createLogger } from './utils/logger'

const settingsStore = useSettingsStore()
const logger = createLogger('GuaxiApp')

const globalNavigationItems = mainNavigationItems.map((item) => ({
  ...item,
  shortcut: GUAXI_SHORTCUTS[item.shortcutId],
}))

let navIconObserver = null
let navIconDecorationFrame = 0

const currentHashPath = () => (window.location.hash || '#/').slice(1) || '/'

const navigateByShortcut = (item) => {
  const from = currentHashPath()
  logger.info('[GlobalNavigationShortcut] navigate requested', {
    from,
    to: item.path,
    label: item.label,
    shortcut: shortcutText(item.shortcut),
  })
  window.location.hash = item.path
}

useGuaxiHotkeys(
  globalNavigationItems.map((item) => ({
    id: `nav:${item.path}`,
    label: item.label,
    shortcut: item.shortcut,
    handler: () => navigateByShortcut(item),
  })),
  {
    logger,
    scope: 'GuaxiGlobalNavigationShortcuts',
  },
)

const decorateNavIcon = (icon, item) => {
  if (!icon || !item) return

  const title = shortcutTooltip(item.label, item.shortcut)
  icon.setAttribute('title', title)
  icon.setAttribute('aria-label', title)
  icon.setAttribute('data-guaxi-nav-shortcut', shortcutText(item.shortcut))
}

const decorateGlobalNavIcons = () => {
  cancelAnimationFrame(navIconDecorationFrame)
  navIconDecorationFrame = requestAnimationFrame(() => {
    const topItems = globalNavigationItems.slice(0, -1)
    const settingItem = globalNavigationItems[globalNavigationItems.length - 1]

    document.querySelectorAll('.sider-icon-container').forEach((container) => {
      container.querySelectorAll('.sider-icon-top .nav-icon').forEach((icon, index) => {
        decorateNavIcon(icon, topItems[index])
      })
      decorateNavIcon(container.querySelector('.sider-icon-bottom .nav-icon'), settingItem)
    })
  })
}

onMounted(async () => {
  await settingsStore.loadAppearanceConfig()
  decorateGlobalNavIcons()
  navIconObserver = new MutationObserver(decorateGlobalNavIcons)
  navIconObserver.observe(document.body, {
    childList: true,
    subtree: true,
  })
  logger.debug('[GlobalNavigationShortcut] icon tooltip decorator mounted', {
    count: globalNavigationItems.length,
  })
})

onBeforeUnmount(() => {
  navIconObserver?.disconnect?.()
  navIconObserver = null
  cancelAnimationFrame(navIconDecorationFrame)
})

const ONE_DARK = {
  bg: '#282c34',
  panel: '#21252b',
  header: '#21252b',
  border: '#3e4451',
  text1: '#abb2bf',
  text2: '#9da5b4',
  primary: '#61afef',
  info: '#56b6c2',
  success: '#98c379',
  warning: '#e5c07b',
  error: '#e06c75',
}

const ONE_DARK_LIGHT = {
  bg: '#ffffff',
  panel: '#ffffff',
  header: '#ffffff',
  border: '#d0d7de',
  text1: '#1f2328',
  text2: '#57606a',
  primary: '#4078f2',
  info: '#0184bc',
  success: '#50a14f',
  warning: '#c18401',
  error: '#e45649',
}

const naiveTheme = computed(() => {
  return settingsStore.appearanceTheme === 'dark' ? darkTheme : null
})

const themeOverrides = computed(() => {
  const p = settingsStore.appearanceTheme === 'dark' ? ONE_DARK : ONE_DARK_LIGHT
  return {
    common: {
      primaryColor: p.primary,
      primaryColorHover: p.primary,
      primaryColorPressed: p.primary,
      infoColor: p.info,
      successColor: p.success,
      warningColor: p.warning,
      errorColor: p.error,
      bodyColor: p.bg,
      cardColor: p.panel,
      modalColor: p.panel,
      popoverColor: p.panel,
      borderColor: p.border,
      textColorBase: p.text1,
      textColor1: p.text1,
      textColor2: p.text2,
    },
  }
})
</script>

<style>
:root {
  --guaxi-editor-font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', 'Courier New';
  --guaxi-app-bg: #ffffff;
  --guaxi-header-bg: #ffffff;
  --guaxi-content-bg: #ffffff;
  --guaxi-panel-bg: #ffffff;
  --guaxi-border: #d0d7de;
  --guaxi-text-primary: #1f2328;
  --guaxi-text-secondary: #57606a;
  --guaxi-header-shadow: 0px 1px 2px rgba(0, 0, 0, 0.06);
  --guaxi-syntax-base: #383a42;
  --guaxi-syntax-comment: #6a737d;
  --guaxi-syntax-string: #22863a;
  --guaxi-syntax-keyword: #a626a4;
  --guaxi-syntax-number: #986801;
  --guaxi-syntax-boolean: #986801;
  --guaxi-syntax-regexp: #007c89;
  --guaxi-syntax-operator: #007c89;
  --guaxi-syntax-punctuation: #383a42;
  --guaxi-syntax-type: #6f42c1;
  --guaxi-syntax-class: #6f42c1;
  --guaxi-syntax-function: #0969da;
  --guaxi-syntax-method: #007c89;
  --guaxi-syntax-property: #007c89;
  --guaxi-syntax-parameter: #9a6700;
  --guaxi-syntax-variable: #0550ae;
  --guaxi-syntax-constant: #986801;
  --guaxi-syntax-namespace: #6f42c1;
  --guaxi-syntax-macro: #a626a4;
  --guaxi-syntax-tag: #e45649;
  --guaxi-syntax-attribute: #9a6700;
  --guaxi-syntax-builtin: #0969da;
  --guaxi-syntax-decorator: #a626a4;
}

:root[data-guaxi-theme='dark'] {
  --guaxi-app-bg: #282c34;
  --guaxi-header-bg: #21252b;
  --guaxi-content-bg: #282c34;
  --guaxi-panel-bg: #21252b;
  --guaxi-border: #3e4451;
  --guaxi-text-primary: #d8e0eb;
  --guaxi-text-secondary: #9da5b4;
  --guaxi-header-shadow: 0px 2px 4px rgba(0, 0, 0, 0.35);
  --guaxi-syntax-base: #abb2bf;
  --guaxi-syntax-comment: #5c6370;
  --guaxi-syntax-string: #98c379;
  --guaxi-syntax-keyword: #c678dd;
  --guaxi-syntax-number: #d19a66;
  --guaxi-syntax-boolean: #d19a66;
  --guaxi-syntax-regexp: #56b6c2;
  --guaxi-syntax-operator: #56b6c2;
  --guaxi-syntax-punctuation: #abb2bf;
  --guaxi-syntax-type: #e5c07b;
  --guaxi-syntax-class: #e5c07b;
  --guaxi-syntax-function: #61afef;
  --guaxi-syntax-method: #56b6c2;
  --guaxi-syntax-property: #56b6c2;
  --guaxi-syntax-parameter: #d19a66;
  --guaxi-syntax-variable: #d7deea;
  --guaxi-syntax-constant: #d19a66;
  --guaxi-syntax-namespace: #e5c07b;
  --guaxi-syntax-macro: #c678dd;
  --guaxi-syntax-tag: #e06c75;
  --guaxi-syntax-attribute: #d19a66;
  --guaxi-syntax-builtin: #56b6c2;
  --guaxi-syntax-decorator: #c678dd;
}

html,
body {
  /*
   * 根布局设计说明：
   * 1. 应用内部的文件树、编辑器等面板各自管理滚动，浏览器文档本身不参与滚动。
   * 2. 清除 body 默认的 8px 外边距，避免页面使用满高布局时产生额外高度。
   * 3. 建立 html -> body -> #app 的完整高度链，页面组件可以稳定继承窗口高度。
   */
  width: 100%;
  height: 100%;
  margin: 0;
  padding: 0;
  overflow: hidden;
  background: var(--guaxi-app-bg);
  color: var(--guaxi-text-primary);
}

#app {
  width: 100%;
  height: 100%;
  min-height: 0;
  overflow: hidden;
}

pre,
.hljs,
.code-segment {
  scrollbar-width: thin;
  scrollbar-color: rgba(0, 0, 0, 0.22) transparent;
}

/* 设计说明：阅读器面板统一使用“滚动时显色、静止时隐藏”的滚动条。
   这样鼠标滚轮、触控板和键盘触发滚动时都能看到滚动位置，同时不影响编辑器/代码块的既有滚动条。 */
.guaxi-reader-scrollbar {
  scrollbar-width: thin;
  scrollbar-color: transparent transparent;
}

.guaxi-reader-scrollbar:hover,
.guaxi-reader-scrollbar:focus-within,
.guaxi-reader-scrollbar.is-reader-scrolling {
  scrollbar-color: rgba(0, 0, 0, 0.32) transparent;
}

pre::-webkit-scrollbar,
.hljs::-webkit-scrollbar,
.code-segment::-webkit-scrollbar,
.guaxi-reader-scrollbar::-webkit-scrollbar {
  width: 6px;
  height: 6px;
}

pre::-webkit-scrollbar-thumb,
.hljs::-webkit-scrollbar-thumb,
.code-segment::-webkit-scrollbar-thumb {
  background-color: rgba(0, 0, 0, 0.22);
  border-radius: 999px;
}

.guaxi-reader-scrollbar::-webkit-scrollbar-thumb {
  background-color: transparent;
  border-radius: 999px;
}

pre::-webkit-scrollbar-thumb:hover,
.hljs::-webkit-scrollbar-thumb:hover,
.code-segment::-webkit-scrollbar-thumb:hover,
.guaxi-reader-scrollbar:hover::-webkit-scrollbar-thumb,
.guaxi-reader-scrollbar:focus-within::-webkit-scrollbar-thumb,
.guaxi-reader-scrollbar.is-reader-scrolling::-webkit-scrollbar-thumb {
  background-color: rgba(0, 0, 0, 0.34);
}

pre::-webkit-scrollbar-track,
.hljs::-webkit-scrollbar-track,
.code-segment::-webkit-scrollbar-track,
.guaxi-reader-scrollbar::-webkit-scrollbar-track {
  background-color: transparent;
}

:root[data-guaxi-theme='dark'] pre,
:root[data-guaxi-theme='dark'] .hljs,
:root[data-guaxi-theme='dark'] .code-segment {
  scrollbar-color: rgba(255, 255, 255, 0.18) transparent;
}

:root[data-guaxi-theme='dark'] .guaxi-reader-scrollbar:hover,
:root[data-guaxi-theme='dark'] .guaxi-reader-scrollbar:focus-within,
:root[data-guaxi-theme='dark'] .guaxi-reader-scrollbar.is-reader-scrolling {
  scrollbar-color: rgba(255, 255, 255, 0.28) transparent;
}

:root[data-guaxi-theme='dark'] pre::-webkit-scrollbar-thumb,
:root[data-guaxi-theme='dark'] .hljs::-webkit-scrollbar-thumb,
:root[data-guaxi-theme='dark'] .code-segment::-webkit-scrollbar-thumb {
  background-color: rgba(255, 255, 255, 0.18);
}

:root[data-guaxi-theme='dark'] pre::-webkit-scrollbar-thumb:hover,
:root[data-guaxi-theme='dark'] .hljs::-webkit-scrollbar-thumb:hover,
:root[data-guaxi-theme='dark'] .code-segment::-webkit-scrollbar-thumb:hover,
:root[data-guaxi-theme='dark'] .guaxi-reader-scrollbar:hover::-webkit-scrollbar-thumb,
:root[data-guaxi-theme='dark'] .guaxi-reader-scrollbar:focus-within::-webkit-scrollbar-thumb,
:root[data-guaxi-theme='dark'] .guaxi-reader-scrollbar.is-reader-scrolling::-webkit-scrollbar-thumb {
  background-color: rgba(255, 255, 255, 0.26);
}

/* 设计说明：普通文本/Markdown Reader 的右侧渲染内容由 HTML 注入，不能依赖单个组件的 scoped CSS 完整命中。
   这里只约束通用文本流和 Markdown 产物，不作用于 PDF/Word/EPUB 左侧专用阅读器的页面渲染。 */
.text-preview-container,
.markdown-panel,
.notes-content {
  overflow-x: hidden;
}

.text-preview-content,
.text-preview-content pre,
.markdown-content,
.markdown-content :where(p, li, blockquote, h1, h2, h3, h4, h5, h6, a, span, div) {
  min-width: 0;
  max-width: 100%;
  overflow-wrap: anywhere;
  word-break: break-word;
}

.text-preview-content,
.text-preview-content pre {
  white-space: pre-wrap;
}

.markdown-content {
  width: 100%;
  box-sizing: border-box;
}

.markdown-content img,
.markdown-content svg,
.markdown-content canvas,
.markdown-content video {
  max-width: 100%;
  height: auto;
  box-sizing: border-box;
}

.markdown-content table {
  width: 100%;
  max-width: 100%;
  table-layout: fixed;
  border-collapse: collapse;
}

.markdown-content th,
.markdown-content td {
  min-width: 0;
  overflow-wrap: anywhere;
  word-break: break-word;
}

.markdown-content pre,
.markdown-content .hljs {
  max-width: 100%;
  box-sizing: border-box;
  white-space: pre-wrap;
  overflow-x: hidden;
  overflow-wrap: anywhere;
}

.markdown-content pre code,
.markdown-content .hljs code {
  white-space: pre-wrap;
  overflow-wrap: anywhere;
  word-break: break-word;
}

.markdown-content mjx-container,
.markdown-content .math-block,
.markdown-content .math-inline {
  max-width: 100%;
  overflow-x: auto;
  overflow-y: hidden;
}

.markdown-content .mermaid {
  max-width: 100%;
  overflow-x: hidden;
}

.markdown-content .mermaid svg {
  max-width: 100%;
  height: auto;
}

:root[data-guaxi-theme='dark'] .fit-width,
:root[data-guaxi-theme='dark'] .zoom-in,
:root[data-guaxi-theme='dark'] .zoom-out {
  background-color: var(--guaxi-header-bg) !important;
  color: var(--guaxi-text-primary) !important;
  border: 1px solid var(--guaxi-border) !important;
}

:root[data-guaxi-theme='dark'] .fit-width:hover,
:root[data-guaxi-theme='dark'] .zoom-in:hover,
:root[data-guaxi-theme='dark'] .zoom-out:hover {
  background-color: var(--guaxi-header-bg) !important;
}

:root[data-guaxi-theme='dark'] .zoom-in:disabled,
:root[data-guaxi-theme='dark'] .zoom-out:disabled {
  background-color: var(--guaxi-header-bg) !important;
  color: var(--guaxi-text-secondary) !important;
}

:root[data-guaxi-theme='dark'] .markdown-panel,
:root[data-guaxi-theme='dark'] .markdown-content {
  background-color: var(--guaxi-content-bg) !important;
  color: var(--guaxi-text-primary) !important;
}

:root[data-guaxi-theme='dark'] .original-content {
  background-color: var(--guaxi-panel-bg) !important;
}

:root[data-guaxi-theme='dark'] .translated-content {
  background-color: var(--guaxi-header-bg) !important;
  color: var(--guaxi-text-secondary) !important;
}

:root[data-guaxi-theme='dark'] .code-segment {
  background: var(--guaxi-header-bg) !important;
}

:root[data-guaxi-theme='dark'] .diagram-segment {
  background: var(--guaxi-panel-bg) !important;
}

:root[data-guaxi-theme='dark'] .note-segment {
  border-bottom: 1px dashed var(--guaxi-border) !important;
}

:root[data-guaxi-theme='dark'] .original-fallback {
  border-top: 1px dashed var(--guaxi-border) !important;
}

:root[data-guaxi-theme='dark'] .skipped-translation {
  border-left: 3px solid var(--guaxi-border) !important;
  color: var(--guaxi-text-secondary) !important;
}

:root[data-guaxi-theme='dark'] .translation-skipped-marker {
  background-color: var(--guaxi-header-bg) !important;
  color: var(--guaxi-text-secondary) !important;
}

:root[data-guaxi-theme='dark'] .hljs {
  background: var(--guaxi-panel-bg) !important;
  border-color: var(--guaxi-border) !important;
  color: var(--guaxi-text-primary) !important;
}

:root[data-guaxi-theme='dark'] .mermaid {
  background-color: #ffffff !important;
  color: #1f2328 !important;
  border: 1px solid var(--guaxi-border) !important;
}

:root[data-guaxi-theme='dark'] .mermaid svg {
  background-color: #ffffff !important;
}
</style>
