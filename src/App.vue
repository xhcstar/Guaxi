<template>
  <n-config-provider :theme="naiveTheme" :theme-overrides="themeOverrides">
    <RouterView />
  </n-config-provider>
</template>

<script setup>
import { computed, onMounted } from 'vue'
import { RouterView } from 'vue-router'
import { NConfigProvider, darkTheme } from 'naive-ui'
import { useSettingsStore } from './stores/settingsStore'

const settingsStore = useSettingsStore()

onMounted(async () => {
  await settingsStore.loadAppearanceConfig()
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
  --guaxi-app-bg: #ffffff;
  --guaxi-header-bg: #ffffff;
  --guaxi-content-bg: #ffffff;
  --guaxi-panel-bg: #ffffff;
  --guaxi-border: #d0d7de;
  --guaxi-text-primary: #1f2328;
  --guaxi-text-secondary: #57606a;
  --guaxi-header-shadow: 0px 1px 2px rgba(0, 0, 0, 0.06);
}

:root[data-guaxi-theme='dark'] {
  --guaxi-app-bg: #282c34;
  --guaxi-header-bg: #21252b;
  --guaxi-content-bg: #282c34;
  --guaxi-panel-bg: #21252b;
  --guaxi-border: #3e4451;
  --guaxi-text-primary: #abb2bf;
  --guaxi-text-secondary: #9da5b4;
  --guaxi-header-shadow: 0px 2px 4px rgba(0, 0, 0, 0.35);
}

html,
body {
  background: var(--guaxi-app-bg);
  color: var(--guaxi-text-primary);
}

pre,
.hljs,
.code-segment {
  scrollbar-width: thin;
  scrollbar-color: rgba(0, 0, 0, 0.22) transparent;
}

pre::-webkit-scrollbar,
.hljs::-webkit-scrollbar,
.code-segment::-webkit-scrollbar {
  width: 6px;
  height: 6px;
}

pre::-webkit-scrollbar-thumb,
.hljs::-webkit-scrollbar-thumb,
.code-segment::-webkit-scrollbar-thumb {
  background-color: rgba(0, 0, 0, 0.22);
  border-radius: 999px;
}

pre::-webkit-scrollbar-thumb:hover,
.hljs::-webkit-scrollbar-thumb:hover,
.code-segment::-webkit-scrollbar-thumb:hover {
  background-color: rgba(0, 0, 0, 0.34);
}

pre::-webkit-scrollbar-track,
.hljs::-webkit-scrollbar-track,
.code-segment::-webkit-scrollbar-track {
  background-color: transparent;
}

:root[data-guaxi-theme='dark'] pre,
:root[data-guaxi-theme='dark'] .hljs,
:root[data-guaxi-theme='dark'] .code-segment {
  scrollbar-color: rgba(255, 255, 255, 0.18) transparent;
}

:root[data-guaxi-theme='dark'] pre::-webkit-scrollbar-thumb,
:root[data-guaxi-theme='dark'] .hljs::-webkit-scrollbar-thumb,
:root[data-guaxi-theme='dark'] .code-segment::-webkit-scrollbar-thumb {
  background-color: rgba(255, 255, 255, 0.18);
}

:root[data-guaxi-theme='dark'] pre::-webkit-scrollbar-thumb:hover,
:root[data-guaxi-theme='dark'] .hljs::-webkit-scrollbar-thumb:hover,
:root[data-guaxi-theme='dark'] .code-segment::-webkit-scrollbar-thumb:hover {
  background-color: rgba(255, 255, 255, 0.26);
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