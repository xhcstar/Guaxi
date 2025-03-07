<template>
  <n-config-provider>
    <div class="app-layout">
      <component :is="currentView" />
    </div>
  </n-config-provider>
</template>


<script setup>
import { ref, computed } from "vue";
// import { routes } from '../router/index.js'
import GuaxiHomePage from '../views/GuaxiHomePage.vue'
import GuaxiChatPage from '../views/GuaxiChatPage.vue'
import GuaxiStatisticsPage from '../views/GuaxiStatisticsPage.vue'
import GuaxiApplicationPage from '../views/GuaxiApplicationPage.vue'
import GuaxiSearchPage from '../views/GuaxiSearchPage.vue'
import GuaxiSettingPage from '../views/GuaxiSettingPage.vue'
import GuaxiRepoPage from '../views/GuaxiRepoPage.vue'
import NotFound from '../views/NotFound.vue'
import { NConfigProvider } from 'naive-ui'

const routes = {
  '/': GuaxiHomePage,
  '/chat': GuaxiChatPage,
  '/repository': GuaxiRepoPage,
  '/statistics': GuaxiStatisticsPage,
  '/application': GuaxiApplicationPage,
  '/search': GuaxiSearchPage,
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
