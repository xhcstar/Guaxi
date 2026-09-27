<template>
  <div class="library-site-page">
    <LibrarySiteApp />
  </div>
</template>

<script setup>
/**
 * 资料库 · 分享与发布 独立窗口(路由 /library-site)。
 *
 * 由 Rust open_library_site_window 打开,URL 带 ?theme=dark|light。
 * 这里把该主题落到 documentElement.dataset.guaxiTheme —— 与 Guaxi 全局
 * 同一个数据源,LibrarySiteApp(内含 iframe)与其 guaxiTheme() 都据此取色,
 * 保证独立窗口、内嵌站点、系统阅读器三处主题一致。
 */
import { onMounted } from 'vue'
import { useRoute } from 'vue-router'
import LibrarySiteApp from '../components/ApplicationViewComponent/apps/LibrarySiteApp.vue'

const route = useRoute()

onMounted(() => {
  const theme = String(route.query.theme || '').trim().toLowerCase()
  if (theme === 'dark' || theme === 'light') {
    document.documentElement.dataset.guaxiTheme = theme
  }
})
</script>

<style scoped>
/* 独立窗口根:用 100vh(与本项目其它独立窗口一致) */
.library-site-page {
  height: 100vh;
  max-height: 100vh;
  overflow: hidden;
  background: var(--guaxi-app-bg);
}
.library-site-page > :deep(.lib-app) {
  height: 100%;
}
</style>
