<template>
  <div class="app-layout">
    <KeepAlive :include="['GuaxiHomePage']">
      <component :is="currentView" />
    </KeepAlive>
  </div>
</template>


<script setup>
import { ref, computed } from "vue";
// import { routes } from '../router/index.js'
import GuaxiHomePage from '../views/GuaxiHomePage.vue'
import GuaxiAgentWorkspacePage from '../views/GuaxiAgentWorkspacePage.vue'
import GuaxiBrainPage from '../views/GuaxiBrainPage.vue'
import GuaxiApplicationPage from '../views/GuaxiApplicationPage.vue'
import GuaxiSearchPage from '../views/GuaxiSearchPage.vue'
import GuaxiSettingPage from '../views/GuaxiSettingPage.vue'
import GuaxiRepoPage from '../views/GuaxiRepoPage.vue'
import GuaxiStatisticsPage from '../views/GuaxiStatisticsPage.vue'
import ExcalidrawPage from '../views/ExcalidrawPage.vue'
import NotFound from '../views/NotFound.vue'

const routes = {
  '/': GuaxiHomePage,
  '/chat': GuaxiAgentWorkspacePage,
  '/repository': GuaxiRepoPage,
  '/brain': GuaxiBrainPage,
  '/application': GuaxiApplicationPage,
  '/search': GuaxiSearchPage,
  '/statistics': GuaxiStatisticsPage,
  '/excalidraw': ExcalidrawPage,
  '/setting': GuaxiSettingPage,
}

const currentPath = ref(window.location.hash)
window.addEventListener('hashchange', () => {
  currentPath.value = window.location.hash
})
const currentView = computed(() => {
  return routes[currentPath.value.slice(1) || '/'] || NotFound
})

</script>


<style scoped>
.app-layout {
  display: flex;
  flex-direction: column;
  height: 100vh;
}
</style>
