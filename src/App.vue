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
  bg: '#fafafa',
  panel: '#ffffff',
  header: '#f5f5f5',
  border: '#e5e5e6',
  text1: '#383a42',
  text2: '#666666',
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
  --guaxi-app-bg: #fafafa;
  --guaxi-header-bg: #f5f5f5;
  --guaxi-content-bg: #ffffff;
  --guaxi-panel-bg: #ffffff;
  --guaxi-border: #e5e5e6;
  --guaxi-text-primary: #383a42;
  --guaxi-text-secondary: #666666;
  --guaxi-header-shadow: 0px 2px 4px rgba(0, 0, 0, 0.1);
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
</style>