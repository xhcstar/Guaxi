import { createApp } from "vue";
import naive from "naive-ui";
import App from "./App.vue";
import router from "./router";
import { createPinia } from 'pinia';
import { initTauriEvents, cleanupTauriEvents } from './stores/tauriEvents';
import { invoke } from '@tauri-apps/api/core'
import { useSettingsStore } from './stores/settingsStore'

const app = createApp(App);

const pinia = createPinia()

app.use(naive);
app.use(router);
app.use(pinia);

const settingsStore = useSettingsStore(pinia)

const getThemeFromUrl = () => {
  try {
    let qs = window.location.search || ''
    if (!qs) {
      const hash = window.location.hash || ''
      const qIdx = hash.indexOf('?')
      if (qIdx >= 0) qs = hash.slice(qIdx)
    }
    const theme = new URLSearchParams(qs).get('theme')
    return theme === 'dark' || theme === 'light' ? theme : null
  } catch {
    return null
  }
}

const theme = getThemeFromUrl()
if (theme) {
  settingsStore.appearanceTheme = theme
  settingsStore.applyAppearanceTheme(theme)
}

// 初始化Tauri事件系统
initTauriEvents().catch(error => {
  console.error('全局事件初始化失败:', error);
});

invoke('init_backend_services').catch(error => {
  console.error('后端服务初始化失败:', error);
});

app.mount("#app");

// 应用卸载时清理
window.addEventListener('beforeunload', async () => {
  await cleanupTauriEvents();
});