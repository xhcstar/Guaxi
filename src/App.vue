<template>
  <!-- <GuaxiMainLayout /> -->
  <RouterView />
  <StealthOverlay ref="screenshot" />
</template>

<script setup>
// import GuaxiMainLayout from './components/GuaxiMainLayout.vue'
import { RouterView } from 'vue-router'
import { onMounted, ref } from 'vue';
import StealthOverlay from './components/ScreenCapture/StealthOverlay.vue';

const screenshot = ref(null);

// 初始化快捷键
onMounted(() => {
  screenshot.value?.setupGlobalShortcut();
});

</script>