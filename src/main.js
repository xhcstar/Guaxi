import { createApp } from "vue";
import naive from "naive-ui";
import App from "./App.vue";
import router from "./router";
import { createPinia } from 'pinia';
import { initTauriEvents, cleanupTauriEvents } from './stores/tauriEvents';
import { invoke } from '@tauri-apps/api/core'

const app = createApp(App);

app.use(naive);
app.use(router);
app.use(createPinia());

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