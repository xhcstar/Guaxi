<template>
  <RouterView />
  <StealthOverlay ref="screenshot" />
</template>

<script setup>
import { RouterView } from 'vue-router'
import { onMounted, ref, onUnmounted } from 'vue';
import StealthOverlay from './components/ScreenCapture/StealthOverlay.vue';

const screenshot = ref(null);

onMounted(() => {
  screenshot.value?.setupGlobalShortcut?.();
});

// 添加全局 ESC 监听
onUnmounted(() => {
  window.removeEventListener('keydown', handleGlobalKeyDown);
});
</script>